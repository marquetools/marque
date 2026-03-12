<?xml version="1.0" encoding="UTF-8"?>
<!--UNCLASSIFIED--><xsl:stylesheet xmlns:xs="http://www.w3.org/2001/XMLSchema"
                xmlns:xsd="http://www.w3.org/2001/XMLSchema"
                xmlns:saxon="http://saxon.sf.net/"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:schold="http://www.ascc.net/xml/schematron"
                xmlns:iso="http://purl.oclc.org/dsdl/schematron"
                xmlns:xhtml="http://www.w3.org/1999/xhtml"
                xmlns:ism="urn:us:gov:ic:ism"
                xmlns:ntk="urn:us:gov:ic:ntk"
                xmlns:arh="urn:us:gov:ic:arh"
                xmlns:catt="urn:us:gov:ic:taxonomy:catt:tetragraph"
                xmlns:cve="urn:us:gov:ic:cve"
                xmlns:dvf="deprecated:value:function"
                xmlns:util="urn:us:gov:ic:ism:xsl:util"
                version="2.0"><!--Implementers: please note that overriding process-prolog or process-root is 
    the preferred method for meta-stylesheets to use where possible. -->
<xsl:param name="archiveDirParameter"/>
   <xsl:param name="archiveNameParameter"/>
   <xsl:param name="fileNameParameter"/>
   <xsl:param name="fileDirParameter"/>
   <xsl:variable name="document-uri">
      <xsl:value-of select="document-uri(/)"/>
   </xsl:variable>

   <!--PHASES-->


<!--PROLOG-->
<xsl:output xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
               method="xml"
               omit-xml-declaration="no"
               standalone="yes"
               indent="yes"/>

   <!--XSD TYPES FOR XSLT2-->


<!--KEYS AND FUNCTIONS-->
<xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:contributesToRollup"
                 as="xs:boolean">
      <xsl:param name="context"/>
      <xsl:sequence select="not(string($context/@ism:excludeFromRollup) = string(true()))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getDissemControlsList"
                 as="node()*">
      <xsl:choose>
         <xsl:when test="($ISM_USGOV_RESOURCE or $ISM_OTHER_AUTH_RESOURCE) and not($ISM_USCUI_RESOURCE)">
            <xsl:copy-of select="document('../../CVE/ISM/CVEnumISMDissemIcrm.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
         </xsl:when>
         <xsl:when test="$ISM_USGOV_RESOURCE and $ISM_USCUI_RESOURCE">
            <xsl:copy-of select="document('../../CVE/ISM/CVEnumISMDissemCommingled.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
         </xsl:when>
         <xsl:when test="$ISM_USCUIONLY_RESOURCE">
            <xsl:copy-of select="document('../../CVE/ISM/CVEnumISMDissemCui.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
         </xsl:when>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="dvf:deprecated"
                 as="xs:string*">
      <xsl:param name="attribute" as="xs:string"/>
      <xsl:param name="depTerms" as="element()*"/>
      <xsl:param name="curDate" as="xs:date?"/>
      <xsl:param name="isError" as="xs:boolean"/>
      
      <xsl:if test="count($curDate) = 1">
         <xsl:for-each select="$depTerms[cve:Value = tokenize($attribute, ' ')]">
            <xsl:if test="($isError and $curDate gt xs:date(@deprecated)) or (not($isError) and $curDate le xs:date(@deprecated))">
               <xsl:sequence select="concat('[', string(current()/cve:Value), '] has been deprecated and is not authorized for use after  ', current()/@deprecated)"/>
            </xsl:if>
         </xsl:for-each>
      </xsl:if>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:containsAnyTokenMatching"
                 as="xs:boolean">
      <xsl:param name="attribute"/>
      <xsl:param name="regexList" as="xs:string+"/>
      <xsl:sequence select="             some $attrToken in tokenize(normalize-space(string($attribute)), ' ')                satisfies (some $regex in $regexList                   satisfies matches($attrToken, $regex))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:containsAnyOfTheTokens"
                 as="xs:boolean">
      <xsl:param name="attribute"/>
      <xsl:param name="tokenList" as="xs:string*"/>
      <xsl:sequence select="             some $attrToken in tokenize(normalize-space(string($attribute)), ' ')                satisfies $attrToken = $tokenList"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:containsOnlyTheTokens"
                 as="xs:boolean">
      <xsl:param name="attribute"/>
      <xsl:param name="tokenList" as="xs:string*"/>
      <xsl:sequence select="             every $attrToken in tokenize(normalize-space(string($attribute)), ' ')                satisfies $attrToken = $tokenList"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:existInTokenSet"
                 as="xs:boolean">
      <xsl:param name="stringTokenValue"/>
      <xsl:param name="tokenList" as="xs:string*"/>
      <xsl:sequence select="tokenize($stringTokenValue, ' ') = $tokenList"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getStringFromSequenceWithOnlyRegexValues"
                 as="xs:string">
      <xsl:param name="attrValues"/>
      <xsl:param name="regex"/>
      <xsl:variable name="StringWithOnlyRegexValues">
         <xsl:for-each select="$attrValues">
            
            <xsl:if test="matches(current(), $regex)">
               <xsl:value-of select="current()"/>
            </xsl:if>
            <xsl:value-of select="' '"/>
         </xsl:for-each>
      </xsl:variable>
      <xsl:sequence select="normalize-space(string($StringWithOnlyRegexValues))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getStringFromSequenceWithoutRegexValues"
                 as="xs:string">
      <xsl:param name="attrValues"/>
      <xsl:param name="regex"/>
      <xsl:variable name="StringWithoutRegexValues">
         <xsl:for-each select="$attrValues">
            
            <xsl:if test="not(matches(current(), $regex))">
               <xsl:value-of select="current()"/>
            </xsl:if>
            <xsl:value-of select="' '"/>
         </xsl:for-each>
      </xsl:variable>
      <xsl:sequence select="normalize-space(string($StringWithoutRegexValues))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getStringFromSequence"
                 as="xs:string">
      <xsl:param name="attrValues"/>
      <xsl:variable name="StringValues">
         <xsl:for-each select="$attrValues">
            <xsl:value-of select="current()"/>
            <xsl:value-of select="' '"/>
         </xsl:for-each>
      </xsl:variable>
      <xsl:sequence select="normalize-space(string($StringValues))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:nonalphabeticValues"
                 as="xs:string">
      <xsl:param name="attrValues"/>
      <xsl:variable name="badValues">
         <xsl:for-each select="$attrValues">
            
            <xsl:if test="not(index-of($attrValues, current())[last()] = count($attrValues))">
               
               <xsl:if test="compare(current(), $attrValues[index-of($attrValues, current()) + 1]) = 1">
                  <xsl:value-of select="$attrValues[index-of($attrValues, current()) + 1]"/>
               </xsl:if>
               <xsl:value-of select="' '"/>
            </xsl:if>
         </xsl:for-each>
      </xsl:variable>
      <xsl:sequence select="normalize-space(string($badValues))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:relativeOrderBetweenACCMAndNonACCMWhenExcludeFromRollup"
                 as="xs:string">
      <xsl:param name="attrValues" as="xs:string*"/>

      <xsl:variable name="badValues">
         <xsl:for-each select="$attrValues">
            
            <xsl:if test="not(index-of($attrValues, current())[last()] = count($attrValues))">
               
               <xsl:if test="not(matches(current(), $ACCMRegex)) and matches($attrValues[index-of($attrValues, current()) + 1], $ACCMRegex) and not(util:existInTokenSet(current(), $nonACCMLeftSetTok))">
                  <xsl:value-of select="current()"/>
               </xsl:if>
               
               <xsl:if test="matches(current(), $ACCMRegex) and not(matches($attrValues[index-of($attrValues, current()) + 1], $ACCMRegex)) and not(util:existInTokenSet($attrValues[index-of($attrValues, current()) + 1], $nonACCMRightSetTok))">
                  <xsl:value-of select="$attrValues[index-of($attrValues, current()) + 1]"/>
               </xsl:if>
               <xsl:value-of select="' '"/>
            </xsl:if>
         </xsl:for-each>
      </xsl:variable>
      <xsl:sequence select="normalize-space(string($badValues))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:unorderedValues"
                 as="xs:string">
      <xsl:param name="attrValues" as="xs:string*"/>
      <xsl:param name="tokenList" as="xs:string*"/>

      <xsl:variable name="badValues">
         <xsl:for-each select="$attrValues">
            
            <xsl:if test="not(index-of($attrValues, current())[last()] = count($attrValues))">

               
               <xsl:variable name="indexOfValue"
                             select="util:getIndexFromListMatch(current(), $tokenList)"/>
               <xsl:variable name="indexOfNextValue"
                             select="util:getIndexFromListMatch($attrValues[index-of($attrValues, current()) + 1], $tokenList)"/>


               <xsl:choose>
                  <xsl:when test="$indexOfValue = $indexOfNextValue">
                     
                     
                     <xsl:if test="compare(current(), $attrValues[index-of($attrValues, current()) + 1]) = 1">
                        <xsl:value-of select="$attrValues[index-of($attrValues, current()) + 1]"/>
                     </xsl:if>
                  </xsl:when>
                  <xsl:when test="$indexOfValue &gt; $indexOfNextValue">
                     
                     <xsl:value-of select="$attrValues[index-of($attrValues, current()) + 1]"/>
                  </xsl:when>
               </xsl:choose>
               <xsl:value-of select="' '"/>
            </xsl:if>
         </xsl:for-each>
      </xsl:variable>
      <xsl:sequence select="normalize-space(string($badValues))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:unsortedValues"
                 as="xs:string">
      <xsl:param name="attribute"/>
      <xsl:param name="tokenList" as="xs:string*"/>
      <xsl:variable name="attrValues"
                    select="tokenize(normalize-space(string($attribute)), ' ')"/>

      <xsl:variable name="badValues">
         <xsl:for-each select="$attrValues">
            
            <xsl:if test="not(index-of($attrValues, current())[last()] = count($attrValues))">

               
               <xsl:variable name="indexOfValue"
                             select="util:getIndexFromListMatch(current(), $tokenList)"/>
               <xsl:variable name="indexOfNextValue"
                             select="util:getIndexFromListMatch($attrValues[index-of($attrValues, current()) + 1], $tokenList)"/>


               <xsl:choose>
                  <xsl:when test="$indexOfValue = $indexOfNextValue">
                     
                     
                     <xsl:if test="compare(current(), $attrValues[index-of($attrValues, current()) + 1]) = 1">
                        <xsl:value-of select="$attrValues[index-of($attrValues, current()) + 1]"/>
                     </xsl:if>
                  </xsl:when>
                  <xsl:when test="$indexOfValue &gt; $indexOfNextValue">
                     
                     <xsl:value-of select="$attrValues[index-of($attrValues, current()) + 1]"/>
                  </xsl:when>
               </xsl:choose>
               <xsl:value-of select="' '"/>
            </xsl:if>
         </xsl:for-each>
      </xsl:variable>
      <xsl:sequence select="normalize-space(string($badValues))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getIndexFromListMatch"
                 as="xs:integer">
      <xsl:param name="value" as="xs:string"/>
      <xsl:param name="list" as="xs:string*"/>

      <xsl:variable name="index">
         <xsl:for-each select="$list">
            <xsl:if test="matches($value, concat('^', current(), '$'))">
               <xsl:value-of select="index-of($list, current())[1]"/>
            </xsl:if>
         </xsl:for-each>
      </xsl:variable>

      <xsl:choose>
         <xsl:when test="$index = ''">
            <xsl:sequence select="xs:integer(-1)"/>
         </xsl:when>
         <xsl:otherwise>
            <xsl:sequence select="xs:integer(number($index[1]))"/>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:meetsType"
                 as="xs:boolean">
      <xsl:param name="value"/>
      <xsl:param name="typePattern" as="xs:string"/>
      <xsl:sequence select="matches(normalize-space(string($value)), concat('^(', $typePattern, ')$'))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getCountriesForTetra"
                 as="xs:string*">
      <xsl:param name="tetra" as="xs:string"/>

      <xsl:sequence select="$decomposableTetraElems[catt:TetraToken/text() = $tetra]/catt:Membership/*/text()"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:padValue"
                 as="xs:string">
      <xsl:param name="value" as="xs:string?"/>

      <xsl:sequence select="concat(' ', normalize-space($value), ' ')"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:tokenize"
                 as="xs:string*">
      <xsl:param name="value" as="xs:string?"/>

      <xsl:sequence select="tokenize(normalize-space($value), ' ')"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:join"
                 as="xs:string">
      <xsl:param name="values" as="xs:string*"/>

      <xsl:sequence select="normalize-space(string-join($values, ' '))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:sort"
                 as="xs:string*">
      <xsl:param name="values" as="xs:string*"/>

      <xsl:variable name="sortedValues">
         <xsl:for-each select="$values">
            <xsl:sort select="."/>
            <xsl:value-of select="util:padValue(.)"/>
         </xsl:for-each>
      </xsl:variable>

      <xsl:sequence select="util:tokenize($sortedValues)"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:countIn"
                 as="xs:double">
      <xsl:param name="value" as="xs:string"/>
      <xsl:param name="expandedRelToStrings" as="xs:string*"/>
      <xsl:param name="countryHash" as="item()*"/>

      <xsl:variable name="counts" as="xs:integer*">
         <xsl:for-each select="$expandedRelToStrings">
            <xsl:if test="util:containsAnyOfTheTokens(., $value)">
               
               <xsl:variable name="expandedPosition" select="position()"/>
               <xsl:sequence select="$countryHash[position() = $expandedPosition * 2]"/>
            </xsl:if>
         </xsl:for-each>
      </xsl:variable>

      <xsl:sequence select="sum($counts)"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:isSubsetOf"
                 as="xs:boolean">
      <xsl:param name="subset" as="xs:string*"/>
      <xsl:param name="superset" as="xs:string*"/>

      <xsl:sequence select="             (every $subsetToken in $subset                satisfies $subsetToken = $superset)"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:containsDecomposableTetra"
                 as="xs:boolean">
      <xsl:param name="relTo" as="xs:string?"/>

      <xsl:sequence select="normalize-space($relTo) and util:containsAnyOfTheTokens($relTo, $decomposableTetras)"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:expandAllTetras"
                 as="xs:string*">
      <xsl:param name="relToStrings" as="xs:string*"/>

      <xsl:variable name="allTokens" as="xs:string*">
         <xsl:for-each select="$relToStrings">
            <xsl:variable name="expandedCountryTokens" select="util:expandDecomposableTetras(.)"/>
            <xsl:value-of select="util:padValue(util:join($expandedCountryTokens))"/>
         </xsl:for-each>
      </xsl:variable>

      <xsl:sequence select="$allTokens"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:expandDecomposableTetras"
                 as="xs:string*">
      <xsl:param name="relTo" as="xs:string"/>

      <xsl:variable name="expandedTetras">
         <xsl:choose>
            <xsl:when test="util:containsDecomposableTetra($relTo)">
               <xsl:variable name="currTetra"
                             select="util:tokenize($relTo)[. = $decomposableTetras][1]"/>
               <xsl:variable name="currTetraCountries"
                             select="util:join(util:getCountriesForTetra($currTetra))"/>
               <xsl:variable name="expandCurrTetra"
                             select="replace(util:padValue($relTo), util:padValue($currTetra), util:padValue($currTetraCountries))"/>

               <xsl:value-of select="util:expandDecomposableTetras($expandCurrTetra)"/>
            </xsl:when>

            <xsl:otherwise>
               <xsl:value-of select="normalize-space($relTo)"/>
            </xsl:otherwise>
         </xsl:choose>
      </xsl:variable>

      <xsl:sequence select="distinct-values(util:tokenize($expandedTetras))[. != 'USA']"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:createCountryHash"
                 as="item()*">
      <xsl:param name="relToStrings" as="xs:string*"/>

      <xsl:for-each-group select="$relToStrings" group-by=".">
         <xsl:sequence select="current-grouping-key(), count(current-group())"/>
      </xsl:for-each-group>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:calculateCommonCountries"
                 as="xs:string*">
      <xsl:param name="portionCountryStrings" as="xs:string*"/>

      
      <xsl:variable name="countryHash"
                    select="util:createCountryHash($portionCountryStrings)"/>

      
      <xsl:variable name="expandedTetras"
                    select="util:expandAllTetras($countryHash[position() mod 2 = 1])"/>
      <xsl:variable name="distinctCountryTokens"
                    select="distinct-values(util:tokenize(util:join($expandedTetras)))[. != 'USA']"/>

      
      <xsl:sequence select="$distinctCountryTokens[util:countIn(., $expandedTetras, $countryHash) = $countFdrPortions]"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:decomposeTetragraphs"
                 as="xs:string*">
      <xsl:param name="releasableTo" as="xs:string"/>
      <xsl:sequence select="             for $token in tokenize(normalize-space($releasableTo), ' ')             return                if (util:isTetragraph($token)) then                   util:getTetragraphMembership($token)                else                   $token"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:isTetragraph"
                 as="xs:boolean">
      <xsl:param name="value" as="xs:string"/>

      <xsl:sequence select="             some $token in $tetragraphList                satisfies $token = $value"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:before-last-delimeter">
      <xsl:param name="s"/>
      <xsl:param name="d"/>

      <xsl:variable name="s-tokenized" select="tokenize($s, $d)"/>
      <xsl:sequence select="string-join(remove($s-tokenized, count($s-tokenized)), $d)"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:containsSpecialTetra"
                 as="xs:boolean">
      <xsl:param name="releasableTo" as="xs:string"/>
      
      <xsl:sequence select="             some $token in tokenize(normalize-space($releasableTo), ' ')                satisfies util:isTetragraph($token) and $catt//catt:Tetragraph[catt:TetraToken = $token]/@decomposable[not(. = 'Yes' or . = 'Maybe' or . = 'NA')]"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:containsMaybeTetra"
                 as="xs:boolean">
      <xsl:param name="releasableTo" as="xs:string"/>
      <xsl:sequence select="             some $token in tokenize(normalize-space($releasableTo), ' ')                satisfies util:isTetragraph($token) and $catt//catt:Tetragraph[catt:TetraToken = $token]/@decomposable[. = 'Maybe']"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:relToContainsMaybeTetra"
                 as="xs:boolean">
      <xsl:param name="bannerRelTo" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count($remainingPartTags) = 0">
            
            <xsl:sequence select="xs:boolean(false())"/>
         </xsl:when>
         <xsl:when test="$bannerRelTo and util:containsMaybeTetra($bannerRelTo)">
            <xsl:sequence select="xs:boolean(true())"/>
         </xsl:when>
         <xsl:when test="$portion/@ism:releasableTo and util:containsMaybeTetra($portion/@ism:releasableTo)">
            <xsl:sequence select="xs:boolean(true())"/>
         </xsl:when>
         <xsl:otherwise>
            <xsl:sequence select="xs:boolean(util:relToContainsMaybeTetraHelper($bannerRelTo, subsequence($remainingPartTags, 2)))"/>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:relToContainsMaybeTetraHelper"
                 as="xs:string*">
      <xsl:param name="bannerRelTo" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count($remainingPartTags) = 1">
            
            <xsl:sequence select="xs:string(util:relToContainsMaybeTetra($bannerRelTo, ()))"/>
         </xsl:when>
         <xsl:otherwise>
            
            <xsl:sequence select="xs:string(util:relToContainsMaybeTetra($bannerRelTo, subsequence($remainingPartTags, 2)))"/>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:displayToContainsMaybeTetra"
                 as="xs:boolean">
      <xsl:param name="bannerDisplayTo" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count($remainingPartTags) = 0">
            
            <xsl:sequence select="xs:boolean(false())"/>
         </xsl:when>
         <xsl:when test="$bannerDisplayTo and util:containsMaybeTetra($bannerDisplayTo)">
            <xsl:sequence select="xs:boolean(true())"/>
         </xsl:when>
         <xsl:when test="$portion/@ism:displayOnlyTo and util:containsMaybeTetra($portion/@ism:displayOnlyTo)">
            <xsl:sequence select="xs:boolean(true())"/>
         </xsl:when>
         <xsl:otherwise>
            <xsl:sequence select="xs:boolean(util:displayToContainsMaybeTetra($bannerDisplayTo, subsequence($remainingPartTags, 2)))"/>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:displayToContainsMaybeTetraHelper"
                 as="xs:string*">
      <xsl:param name="bannerDisplayTo" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count($remainingPartTags) = 1">
            
            <xsl:sequence select="xs:string(util:displayToContainsMaybeTetra($bannerDisplayTo, ()))"/>
         </xsl:when>
         <xsl:otherwise>
            
            <xsl:sequence select="xs:string(util:displayToContainsMaybeTetra($bannerDisplayTo, subsequence($remainingPartTags, 2)))"/>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:bannerIsSubset"
                 as="xs:boolean">
      <xsl:param name="bannerRelTo" as="xs:string"/>
      <xsl:param name="portionRelTo" as="xs:string"/>
      <xsl:variable name="bannerRelToDecomposed"
                    select="tokenize(normalize-space(util:decomposeTetragraphs($bannerRelTo)), ' ')"/>
      <xsl:variable name="portionRelToDecomposed"
                    select="tokenize(normalize-space(util:decomposeTetragraphs($portionRelTo)), ' ')"/>
      <xsl:sequence select="             util:containsSpecialTetra($bannerRelTo) or (every $bannerToken in $bannerRelToDecomposed                satisfies (some $portionToken in $portionRelToDecomposed                   satisfies if ($bannerToken = 'USA') then                      true()                   else                      $bannerToken = $portionToken))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:containsFDR"
                 as="xs:boolean">
      <xsl:param name="elementNode" as="node()"/>
      <xsl:sequence select="$elementNode/@ism:releasableTo or $elementNode/@ism:displayOnlyTo or util:containsAnyOfTheTokens($elementNode/@ism:disseminationControls, ('NF', 'RELIDO')) or util:containsAnyOfTheTokens($elementNode/@ism:nonICmarkings, ('LES-NF', 'SBU-NF'))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:intersectionOfCountries"
                 as="xs:string*">
      <xsl:param name="commonCountries" as="xs:string"/>
      <xsl:param name="portionRelTo" as="xs:string"/>
      <xsl:variable name="portionRelToDecomposed"
                    select="tokenize(normalize-space(util:decomposeTetragraphs($portionRelTo)), ' ')"/>
      <xsl:sequence select="             for $token in tokenize(normalize-space($commonCountries), ' ')             return                if ($token = $portionRelToDecomposed and not($token = 'USA')) then                   $token                else                   ()"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:recursivelyCheckRelTo"
                 as="xs:string*">
      <xsl:param name="bannerRelTo" as="xs:string"/>
      <xsl:param name="commonCountries" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count(tokenize($commonCountries, ' ')) = 0">
            
            <xsl:sequence select="()"/>
         </xsl:when>
         <xsl:when test="count($remainingPartTags) = 0">
            
            <xsl:sequence select="$commonCountries"/>
         </xsl:when>
         <xsl:when test="not(util:containsFDR($portion)) and $portion/@ism:classification = 'U'">
            
            <xsl:sequence select="util:recursivelyCheckRelTo($bannerRelTo, $commonCountries, subsequence($remainingPartTags, 2))"/>
         </xsl:when>
         <xsl:when test="not($portion/@ism:releasableTo)">
            
            <xsl:sequence select="()"/>
         </xsl:when>
         <xsl:when test="util:containsSpecialTetra($portion/@ism:releasableTo)">
            
            <xsl:sequence select="util:recursivelyCheckRelTo($bannerRelTo, $commonCountries, subsequence($remainingPartTags, 2))"/>
         </xsl:when>
         <xsl:otherwise>
            
            <xsl:choose>
               <xsl:when test="util:bannerIsSubset($bannerRelTo, $portion/@ism:releasableTo)">
                  
                  <xsl:sequence select="util:recursivelyCheckRelToRecurseHelper($bannerRelTo, $commonCountries, $remainingPartTags)"/>
               </xsl:when>
               <xsl:otherwise>
                  
                  <xsl:sequence select="('BANNER_NOT_A_SUBSET_OF_A_PORTION')"/>
               </xsl:otherwise>
            </xsl:choose>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:recursivelyCheckRelToRecurseHelper"
                 as="xs:string*">
      <xsl:param name="bannerRelTo" as="xs:string"/>
      <xsl:param name="commonCountries" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count($remainingPartTags) = 1">
            
            <xsl:sequence select="util:recursivelyCheckRelTo($bannerRelTo, util:intersectionOfCountries($commonCountries, $portion/@ism:releasableTo), ())"/>
         </xsl:when>
         <xsl:otherwise>
            
            <xsl:sequence select="util:recursivelyCheckRelTo($bannerRelTo, util:intersectionOfCountries($commonCountries, $portion/@ism:releasableTo), subsequence($remainingPartTags, 2))"/>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:isUncaveatedAndNoFDR"
                 as="xs:boolean">
      <xsl:param name="element"/>
      <xsl:sequence select="not($element/@ism:disseminationControls) and not($element/@ism:SCIcontrols) and not($element/@ism:nonICmarkings) and not($element/@ism:atomicEnergyMarkings) and not($element/@ism:FGIsourceOpen) and not($element/@ism:FGIsourceProtected) and not($element/@ism:nonUSControls) and not($element/@ism:SARIdentifier)"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:checkRelToPortionsAgainstBannerAndGetCommonCountries"
                 as="xs:string*">
      <xsl:param name="bannerRelTo" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count($remainingPartTags) = 0">
            
            <xsl:sequence select="('PASS')"/>
         </xsl:when>
         <xsl:when test="util:containsFDR($portion) and not($portion/@ism:releasableTo)">
            

            <xsl:sequence select="()"/>
         </xsl:when>
         <xsl:when test="$portion/@ism:releasableTo and not(util:containsSpecialTetra($portion/@ism:releasableTo))">
            
            <xsl:sequence select="util:recursivelyCheckRelTo($bannerRelTo, util:decomposeTetragraphs($portion/@ism:releasableTo), $remainingPartTags)"/>

         </xsl:when>
         <xsl:otherwise>
            
            <xsl:sequence select="util:checkRelToPortionsAgainstBannerAndGetCommonCountries($bannerRelTo, subsequence($remainingPartTags, 2))"/>

         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getDisplayToCountries">
      <xsl:param name="portion" as="node()"/>
      <xsl:sequence select="normalize-space(concat(normalize-space(string($portion/@ism:releasableTo)), ' ', normalize-space(string($portion/@ism:displayOnlyTo))))"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:isDisplayable"
                 as="xs:boolean">
      <xsl:param name="portion" as="node()"/>
      <xsl:sequence select="$portion/@ism:releasableTo or $portion/@ism:displayOnlyTo"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:recursivelyCheckDisplayTo"
                 as="xs:string*">
      <xsl:param name="bannerRelToAndDisplayTo" as="xs:string"/>
      <xsl:param name="commonCountries" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count(tokenize($commonCountries, ' ')) = 0">
            
            <xsl:sequence select="()"/>
         </xsl:when>
         <xsl:when test="count($remainingPartTags) = 0">
            
            <xsl:sequence select="$commonCountries"/>
         </xsl:when>
         <xsl:when test="not(util:containsFDR($portion)) and $portion/@ism:classification = 'U'">
            
            <xsl:sequence select="util:recursivelyCheckDisplayTo($bannerRelToAndDisplayTo, $commonCountries, subsequence($remainingPartTags, 2))"/>
         </xsl:when>
         <xsl:when test="not($portion/@ism:releasableTo) and not($portion/@ism:displayOnlyTo)">
            
            <xsl:sequence select="()"/>
         </xsl:when>
         <xsl:when test="util:containsSpecialTetra(util:getDisplayToCountries($portion))">
            
            <xsl:sequence select="util:recursivelyCheckDisplayTo($bannerRelToAndDisplayTo, $commonCountries, subsequence($remainingPartTags, 2))"/>
         </xsl:when>
         <xsl:otherwise>
            
            <xsl:choose>
               <xsl:when test="util:bannerIsSubset($bannerRelToAndDisplayTo, util:getDisplayToCountries($portion))">
                  
                  <xsl:sequence select="util:recursivelyCheckDisplayToRecurseHelper($bannerRelToAndDisplayTo, $commonCountries, $remainingPartTags)"/>
               </xsl:when>
               <xsl:otherwise>
                  
                  <xsl:sequence select="('BANNER_NOT_A_SUBSET_OF_A_PORTION')"/>
               </xsl:otherwise>
            </xsl:choose>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:recursivelyCheckDisplayToRecurseHelper"
                 as="xs:string*">
      <xsl:param name="bannerRelToAndDisplayTo" as="xs:string"/>
      <xsl:param name="commonCountries" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count($remainingPartTags) = 1">
            
            <xsl:sequence select="util:recursivelyCheckDisplayTo($bannerRelToAndDisplayTo, util:intersectionOfCountries($commonCountries, util:getDisplayToCountries($portion)), ())"/>
         </xsl:when>
         <xsl:otherwise>
            
            <xsl:sequence select="util:recursivelyCheckDisplayTo($bannerRelToAndDisplayTo, util:intersectionOfCountries($commonCountries, util:getDisplayToCountries($portion)), subsequence($remainingPartTags, 2))"/>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:checkDisplayToPortionsAgainstBannerAndGetCommonCountries"
                 as="xs:string*">
      <xsl:param name="bannerRelToAndDisplayTo" as="xs:string"/>
      <xsl:param name="remainingPartTags" as="node()*"/>

      <xsl:variable name="portion" select="$remainingPartTags[1]"/>

      <xsl:choose>
         <xsl:when test="count($remainingPartTags) = 0">
            
            <xsl:sequence select="('PASS')"/>
         </xsl:when>
         <xsl:when test="util:containsFDR($portion) and not(util:isDisplayable($portion))">
            
            <xsl:sequence select="()"/>
         </xsl:when>
         <xsl:when test="util:isDisplayable($portion) and not(util:containsSpecialTetra(util:getDisplayToCountries($portion)))">
            
            <xsl:sequence select="util:recursivelyCheckDisplayTo($bannerRelToAndDisplayTo, util:decomposeTetragraphs(util:getDisplayToCountries($portion)), $remainingPartTags)"/>
         </xsl:when>
         <xsl:otherwise>
            
            <xsl:sequence select="util:checkDisplayToPortionsAgainstBannerAndGetCommonCountries($bannerRelToAndDisplayTo, subsequence($remainingPartTags, 2))"/>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getTetragraphMembership">
      <xsl:param name="tetra"/>
      <xsl:variable name="tetragraph"
                    select="$catt//catt:Tetragraph[catt:TetraToken = $tetra]"/>
      <xsl:value-of select="             if ($tetragraph[@decomposable = 'Yes' or @decomposable = 'NA'])             then                string-join(($tetragraph/catt:Membership/*/text()), ' ')             else                $tetra"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:getTetragraphReleasability">
      <xsl:param name="tetra"/>
      <xsl:value-of select="             string-join(distinct-values(for $token in tokenize($catt//catt:Tetragraph[catt:TetraToken = $tetra]/@ism:releasableTo, ' ')             return                if (index-of($catt//catt:TetraToken, $token) &gt; 0) then                   util:getTetragraphMembership($token)                else                   $token), ' ')"/>
   </xsl:function>
   <xsl:function xmlns:sch="http://purl.oclc.org/dsdl/schematron"
                 name="util:countSARmarkings">
      <xsl:param name="sars"/>

      <xsl:variable name="tokenizedSARs" select="tokenize($sars,' ')"/>

      <xsl:variable name="SARmarkings">

         <xsl:for-each select="$tokenizedSARs">

            <xsl:if test="not(position() = 1)">
               <xsl:text> </xsl:text>
            </xsl:if>

            <xsl:variable name="SARlessOwner" select="substring-after(.,':')"/>

            <xsl:choose>
               <xsl:when test="contains($SARlessOwner, ':')">
                  <xsl:value-of select="concat(substring-before(.,':'),':',substring-after($SARlessOwner,':'))"/>
               </xsl:when>
               <xsl:otherwise>
                  <xsl:value-of select="."/>
               </xsl:otherwise>
            </xsl:choose>
         </xsl:for-each>
      </xsl:variable>

      <xsl:value-of select="count(distinct-values(tokenize($SARmarkings,' ')))"/>
   </xsl:function>

   <!--DEFAULT RULES-->


<!--MODE: SCHEMATRON-SELECT-FULL-PATH-->
<!--This mode can be used to generate an ugly though full XPath for locators-->
<xsl:template match="*" mode="schematron-select-full-path">
      <xsl:apply-templates select="." mode="schematron-get-full-path"/>
   </xsl:template>

   <!--MODE: SCHEMATRON-FULL-PATH-->
<!--This mode can be used to generate an ugly though full XPath for locators-->
<xsl:template match="*" mode="schematron-get-full-path">
      <xsl:apply-templates select="parent::*" mode="schematron-get-full-path"/>
      <xsl:text>/</xsl:text>
      <xsl:choose>
         <xsl:when test="namespace-uri()=''">
            <xsl:value-of select="name()"/>
         </xsl:when>
         <xsl:otherwise>
            <xsl:text>*:</xsl:text>
            <xsl:value-of select="local-name()"/>
            <xsl:text>[namespace-uri()='</xsl:text>
            <xsl:value-of select="namespace-uri()"/>
            <xsl:text>']</xsl:text>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:variable name="preceding"
                    select="count(preceding-sibling::*[local-name()=local-name(current())                                   and namespace-uri() = namespace-uri(current())])"/>
      <xsl:text>[</xsl:text>
      <xsl:value-of select="1+ $preceding"/>
      <xsl:text>]</xsl:text>
   </xsl:template>
   <xsl:template match="@*" mode="schematron-get-full-path">
      <xsl:apply-templates select="parent::*" mode="schematron-get-full-path"/>
      <xsl:text>/</xsl:text>
      <xsl:choose>
         <xsl:when test="namespace-uri()=''">@<xsl:value-of select="name()"/>
         </xsl:when>
         <xsl:otherwise>
            <xsl:text>@*[local-name()='</xsl:text>
            <xsl:value-of select="local-name()"/>
            <xsl:text>' and namespace-uri()='</xsl:text>
            <xsl:value-of select="namespace-uri()"/>
            <xsl:text>']</xsl:text>
         </xsl:otherwise>
      </xsl:choose>
   </xsl:template>

   <!--MODE: SCHEMATRON-FULL-PATH-2-->
<!--This mode can be used to generate prefixed XPath for humans-->
<xsl:template match="node() | @*" mode="schematron-get-full-path-2">
      <xsl:for-each select="ancestor-or-self::*">
         <xsl:text>/</xsl:text>
         <xsl:value-of select="name(.)"/>
         <xsl:if test="preceding-sibling::*[name(.)=name(current())]">
            <xsl:text>[</xsl:text>
            <xsl:value-of select="count(preceding-sibling::*[name(.)=name(current())])+1"/>
            <xsl:text>]</xsl:text>
         </xsl:if>
      </xsl:for-each>
      <xsl:if test="not(self::*)">
         <xsl:text/>/@<xsl:value-of select="name(.)"/>
      </xsl:if>
   </xsl:template>
   <!--MODE: SCHEMATRON-FULL-PATH-3-->
<!--This mode can be used to generate prefixed XPath for humans 
	(Top-level element has index)-->
<xsl:template match="node() | @*" mode="schematron-get-full-path-3">
      <xsl:for-each select="ancestor-or-self::*">
         <xsl:text>/</xsl:text>
         <xsl:value-of select="name(.)"/>
         <xsl:if test="parent::*">
            <xsl:text>[</xsl:text>
            <xsl:value-of select="count(preceding-sibling::*[name(.)=name(current())])+1"/>
            <xsl:text>]</xsl:text>
         </xsl:if>
      </xsl:for-each>
      <xsl:if test="not(self::*)">
         <xsl:text/>/@<xsl:value-of select="name(.)"/>
      </xsl:if>
   </xsl:template>

   <!--MODE: GENERATE-ID-FROM-PATH -->
<xsl:template match="/" mode="generate-id-from-path"/>
   <xsl:template match="text()" mode="generate-id-from-path">
      <xsl:apply-templates select="parent::*" mode="generate-id-from-path"/>
      <xsl:value-of select="concat('.text-', 1+count(preceding-sibling::text()), '-')"/>
   </xsl:template>
   <xsl:template match="comment()" mode="generate-id-from-path">
      <xsl:apply-templates select="parent::*" mode="generate-id-from-path"/>
      <xsl:value-of select="concat('.comment-', 1+count(preceding-sibling::comment()), '-')"/>
   </xsl:template>
   <xsl:template match="processing-instruction()" mode="generate-id-from-path">
      <xsl:apply-templates select="parent::*" mode="generate-id-from-path"/>
      <xsl:value-of select="concat('.processing-instruction-', 1+count(preceding-sibling::processing-instruction()), '-')"/>
   </xsl:template>
   <xsl:template match="@*" mode="generate-id-from-path">
      <xsl:apply-templates select="parent::*" mode="generate-id-from-path"/>
      <xsl:value-of select="concat('.@', name())"/>
   </xsl:template>
   <xsl:template match="*" mode="generate-id-from-path" priority="-0.5">
      <xsl:apply-templates select="parent::*" mode="generate-id-from-path"/>
      <xsl:text>.</xsl:text>
      <xsl:value-of select="concat('.',name(),'-',1+count(preceding-sibling::*[name()=name(current())]),'-')"/>
   </xsl:template>

   <!--MODE: GENERATE-ID-2 -->
<xsl:template match="/" mode="generate-id-2">U</xsl:template>
   <xsl:template match="*" mode="generate-id-2" priority="2">
      <xsl:text>U</xsl:text>
      <xsl:number level="multiple" count="*"/>
   </xsl:template>
   <xsl:template match="node()" mode="generate-id-2">
      <xsl:text>U.</xsl:text>
      <xsl:number level="multiple" count="*"/>
      <xsl:text>n</xsl:text>
      <xsl:number count="node()"/>
   </xsl:template>
   <xsl:template match="@*" mode="generate-id-2">
      <xsl:text>U.</xsl:text>
      <xsl:number level="multiple" count="*"/>
      <xsl:text>_</xsl:text>
      <xsl:value-of select="string-length(local-name(.))"/>
      <xsl:text>_</xsl:text>
      <xsl:value-of select="translate(name(),':','.')"/>
   </xsl:template>
   <!--Strip characters--><xsl:template match="text()" priority="-1"/>

   <!--SCHEMA SETUP-->
<xsl:template match="/">
      <svrl:schematron-output xmlns:svrl="http://purl.oclc.org/dsdl/svrl" title="" schemaVersion="">
         <xsl:attribute name="phase">STRUCTURECHECK</xsl:attribute>
         <xsl:comment>
            <xsl:value-of select="$archiveDirParameter"/>   
		 <xsl:value-of select="$archiveNameParameter"/>  
		 <xsl:value-of select="$fileNameParameter"/>  
		 <xsl:value-of select="$fileDirParameter"/>
         </xsl:comment>
         <svrl:text> This is the root file for
      the specifications Schematron ruleset. It loads all of the required CVEs, declares some
      variables, and includes all of the Rule .sch files.</svrl:text>
         <svrl:ns-prefix-in-attribute-values uri="urn:us:gov:ic:ism" prefix="ism"/>
         <svrl:ns-prefix-in-attribute-values uri="urn:us:gov:ic:ntk" prefix="ntk"/>
         <svrl:ns-prefix-in-attribute-values uri="urn:us:gov:ic:arh" prefix="arh"/>
         <svrl:ns-prefix-in-attribute-values uri="urn:us:gov:ic:taxonomy:catt:tetragraph" prefix="catt"/>
         <svrl:ns-prefix-in-attribute-values uri="urn:us:gov:ic:cve" prefix="cve"/>
         <svrl:ns-prefix-in-attribute-values uri="deprecated:value:function" prefix="dvf"/>
         <svrl:ns-prefix-in-attribute-values uri="urn:us:gov:ic:ism:xsl:util" prefix="util"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00405</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00405</xsl:attribute>
            <svrl:text>
        [ISM-ID-00405][Error] The Access Profile Value must not have an @ntk:qualifier attribute specified
        for MN NTK assertions.
    </svrl:text>
            <svrl:text>
        Given an MN NTK assertion (ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:mn'), the ntk:AccessProfileValue/@ntk:qualifier
        attribute is not allowed.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M198"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00406</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00406</xsl:attribute>
            <svrl:text>
        [ISM-ID-00406][Error] If Vocabulary Type is specified in an MN NTK assertion, it must specify 
        a version for either the issue (datasphere:mn:issue) or region (datasphere:mn:region) vocabularies.
    </svrl:text>
            <svrl:text>
        If an ntk:VocabularyType element exists in an MN NTK assertion 
        (ntk:VocabularyType[../ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:mn']), then 
        (1) @ntk:name must be ‘datasphere:mn:issue’ or ‘datasphere:mn:region’ and 
        (2) the @ntk:sourceVersion attribute is required.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M199"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00408</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00408</xsl:attribute>
            <svrl:text>
        [ISM-ID-00408][Error] Propin NTK assertions that use the urn:us:gov:ic:aces:ntk:propin:2 access policy 
        MUST specify a Profile DES.
    </svrl:text>
            <svrl:text>
        If an ntk:AccessProfile has an ntk:AccessPolicy element that has a value of ‘urn:us:gov:ic:aces:ntk:propin:2’, 
        then an ntk:ProfileDes MUST be specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M201"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00416</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00416</xsl:attribute>
            <svrl:text>
        [ISM-ID-00416][Error] If ntk:AccessProfileValue or ntk:VocabularyType are specified 
        then there must be a Profile DES that defines the use of the ntk:AccessProfile structure.
    </svrl:text>
            <svrl:text>
        When there is content in an ntk:AccessProfile, either ntk:AccessProfileValue or ntk:VocabularyType, 
        then there must also be a ntk:ProfileDes in the AccessProfile.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M209"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00417</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00417</xsl:attribute>
            <svrl:text>
        [ISM-ID-00417][Error] If there is a Profile DES specified, then there must be at least
        one ntk:AccessProfileValue.
    </svrl:text>
            <svrl:text>
        When ntk:ProfileDes exists, make sure there is also a following sibling ntk:AccessProfileValue.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M210"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00419</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00419</xsl:attribute>
            <svrl:text>
        [ISM-ID-00419][Error] ntk:AccessProfile containing the ntk:AccessPolicy [urn:us:gov:ic:aces:ntk:ico] may not have
        ntk:ProfileDes, ntk:VocabularyType, or ntk:AccessProfileValue elements specified.
        
        Human Readable: When the ICO ACES is referenced, no data content may be specified in the ntk:AccessProfile.
    </svrl:text>
            <svrl:text>
        For every ntk:AccessProfile that has an ntk:AccessPolicy of [urn:us:gov:ic:aces:ntk:ico], 
        the profile should not specify any of the following data elements, ntk:ProfileDes, ntk:VocabularyType, 
        or ntk:AccessProfileValue.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M212"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00421</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00421</xsl:attribute>
            <svrl:text>
        [ISM-ID-00421][Error] An Agency Dissemination NTK must have one and only one entry
        qualified as the originator.
    </svrl:text>
            <svrl:text>
        For every ntk:AccessProfile with an ntk:ProfileDes of [urn:us:gov:ic:ntk:profile:agencydissem], this rule ensures
        that it has one and only one ntk:AccessProfileValue element with an @ntk:qualifier of
        [originator].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M214"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00422</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00422</xsl:attribute>
            <svrl:text>
      Abstract pattern to require an ntk:VocabularyType with @ntk:sourceVersion for a specified vocabulary.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M215"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00425</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00425</xsl:attribute>
            <svrl:text>
      Abstract pattern to require an ntk:VocabularyType with @ntk:sourceVersion for a specified vocabulary.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M218"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00426</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00426</xsl:attribute>
            <svrl:text>
      Abstract pattern to require an ntk:VocabularyType with @ntk:sourceVersion for a specified vocabulary.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M219"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00437</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00437</xsl:attribute>
            <svrl:text>
      Abstract pattern to require an ntk:VocabularyType with @ntk:sourceVersion for a specified vocabulary.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M230"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00454</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00454</xsl:attribute>
            <svrl:text>
      Abstract pattern to require an ntk:VocabularyType with @ntk:sourceVersion for a specified vocabulary.</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M234"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00455</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00455</xsl:attribute>
            <svrl:text>
        [ISM-ID-00455][Error] ntk:RequiresAnyOf and ntk:RequiresAllOf must contain ntk:AccessProfileList.
        
        Human Readable: ntk:RequiresAnyOf and ntk:RequiresAllOf must have the child element ntk:AccessProfileList.
    </svrl:text>
            <svrl:text>
        This rule ensures that ntk:AccessProfileList exist as a child element of ntk:RequiresAnyOf and ntk:RequiresAllOf.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M235"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00157</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00157</xsl:attribute>
            <svrl:text>
        [ISM-ID-00157][Error] If ISM_USDOD_RESOURCE and: 
        1. The attribute notice contains one of the [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], or [DoD-Dist-E] 
          AND
        2. The attribute @ism:noticeReason is not specified. 
        
        Human Readable: DoD distribution statements B, C, D , or E all require a reason. 
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USDOD_RESOURCE, for each element which
        specifies attribute ism:noticeType with a value containing the token [DoD-Dist-B],
        [DoD-Dist-C], [DoD-Dist-D], or [DoD-Dist-E], this rule ensures that attribute
        @ism:noticeReason is specified. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M246"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00161</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00161</xsl:attribute>
            <svrl:text>
        [ISM-ID-00161][Error] If the document is an
        1. ISM_USDOD_RESOURCE AND
        2. the attribute @ism:noticeType of ISM_RESOURCE_ELEMENT contains [DoD-Dist-A] AND
        3. no portions in the document have their attribute @ism:excludeFromRollup set to [true]
        THEN there must not be any attribute @ism:nonICmarkings present.
        
        Human Readable: Distribution statement A (Public Release) is 
        incompatible with any nonICMarkings if excludeFromRollup is not TRUE.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USDOD_RESOURCE and @ism:noticeType contains 'DoD-Dist-A' 
        and no portions in the document have their @ism:excludeFromRollup set to true, 
        then there must not be any @ism:nonICMarkings present.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M248"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00237</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00237</xsl:attribute>
            <svrl:text>
        [ISM-ID-00237][Error] If ISM_USDOD_RESOURCE, any element which specifies
        attribute @ism:noticeType containing one of the tokens [DoD-Dist-B], 
       	[DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F]
       	must also specify attribute @ism:noticeDate.     	
        
        Human Readable: DoD distribution statements B, C, D, E, and F all require a date.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:noticeType specified with a value containing the token
        [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F], 
        this rule ensures that attribute @ism:noticeDate is specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M251"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00239</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00239</xsl:attribute>
            <svrl:text>
		[ISM-ID-00239][Error] If ISM_USDOD_RESOURCE and attribute @ism:noticeType of
		ISM_RESOURCE_ELEMENT contains the token [DoD-Dist-A], then any element 
		which contributes to rollup should not have an attribute
		@ism:disseminationControls present.
		
		Human Readable: Distribution statement A (Public Release) is incompatible 
		with @ism:disseminationControls present for contributing portions.
	</svrl:text>
            <svrl:text>
		If the document is an ISM_USDOD_RESOURCE and the attribute
		@ism:noticeType of ISM_RESOURCE_ELEMENT contains the token [DoD-Dist-A], for
		each element which specifies attribute @ism:disseminationControls 
		this rule ensures that attribute @ism:disseminationControls is not present.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M253"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00240</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00240</xsl:attribute>
            <svrl:text>
        [ISM-ID-00240][Error] If ISM_USDOD_RESOURCE and attribute @ism:noticeType of
        ISM_RESOURCE_ELEMENT contains the token [DoD-Dist-A], then any element
        which contributes to rollup should not have an attribute
        @ism:atomicEnergyMarkings present.
        
        Human Readable: Distribution statement A (Public Release) is incompatible 
        with @ism:atomicEnergyMarkings.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USDOD_RESOURCE and the attribute
    	@ism:noticeType of ISM_RESOURCE_ELEMENT contains the token [DoD-Dist-A], for
    	each element which specifies attribute @ism:atomicEnergyMarkings this rule ensures that attribute 
    	@ism:atomicEnergyMarkings is not present.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M254"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00527</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00527</xsl:attribute>
            <svrl:text>
		[ISM-ID-00527][Warning] All resource elements that contain a DoD @ism:SARIdentifier attribute SHOULD contain attribute
		@ism:declassException.
	</svrl:text>
            <svrl:text>
	  	For all resource elements which contain a DoD @ism:SARIdentifier attribute, this rule raises a WARNING flag that the 
	  	resource element SHOULD also have an @ism:declassException attribute.  DoD SARs are identified by an @ism:SARIdentifier that
	  	starts with 'SAR-DOD:'.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M255"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00014</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00014</xsl:attribute>
            <svrl:text>
        [ISM-ID-00014][Error] If ISM_NSI_EO_APPLIES then one or more of the following 
        attributes: @ism:declassDate, @ism:declassEvent, or @ism:declassException must be specified on the ISM_RESOURCE_ELEMENT.
        Human Readable: Documents under E.O. 13526 must have declassification instructions included in the 
        classification authority block information.
    </svrl:text>
            <svrl:text>
        If ISM_NSI_EO_APPLIES, this rule ensures that the ISM_RESOURCE_ELEMENT specifies
        one of the following attributes: @ism:declassDate, @ism:declassEvent, @ism:declassException.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M256"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00016</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00016</xsl:attribute>
            <svrl:text>
        [ISM-ID-00016][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:classification has a value of [U], then attributes @ism:classificationReason,
        @ism:classifiedBy, @ism:derivativelyClassifiedBy, @ism:declassDate, @ism:declassEvent, 
        @ism:declassException, @ism:derivedFrom, @ism:SARIdentifier, or @ism:SCIcontrols must not be specified.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:classification specified with a value of [U] this rule ensures that NONE of the following attributes 
    	are specified: @ism:classifiedBy, @ism:declassDate, @ism:declassEvent, @ism:declassException,
    	@ism:derivativelyClassifiedBy, @ism:derivedFrom, @ism:SARIdentifier, or @ism:SCIcontrols. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M257"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00017</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00017</xsl:attribute>
            <svrl:text>
        [ISM-ID-00017][Error] If ISM_NSI_EO_APPLIES and attribute 
        @ism:classifiedBy is specified, then attribute @ism:classificationReason must be specified.         
        Human Readable: Documents under E.O. 13526 containing Originally Classified data require a
        classification reason to be identified.
    </svrl:text>
            <svrl:text>
    	If ISM_NSI_EO_APPLIES, for each element which specifies attribute @ism:classifiedBy, 
    	this rule ensures that attribute @ism:classificationReason is specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M258"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00031</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00031</xsl:attribute>
            <svrl:text>
        [ISM-ID-00031][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls contains the name token [REL] or [EYES], then 
        attribute @ism:releasableTo must be specified. 
        Human Readable: USA documents containing REL TO or EYES ONLY 
        dissemination must specify to which countries the document is releasable.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [REL] or [EYES] this rule ensures that attribute @ism:releasableTo
    	is specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M261"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00032</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00032</xsl:attribute>
            <svrl:text>
        [ISM-ID-00032][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls is not specified, or is specified and does not 
        contain the name token [REL] or [EYES], then attribute @ism:releasableTo 
        must not be specified.
        
        Human Readable: USA documents must only specify to which countries it is 
        authorized for release if dissemination information contains 
        REL TO or EYES ONLY data. 
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which
        does not specify attribute @ism:disseminationControls or specifies attribute
        @ism:disseminationControls with a value containing the token 
        [REL] or [EYES] this rule ensures that attribute @ism:releasableTo is not 
        specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M262"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00064</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00064</xsl:attribute>
            <svrl:text>
        [ISM-ID-00064][Error] If ISM_USGOV_RESOURCE and any element meeting
        ISM_CONTRIBUTES in the document have the attribute @ism:FGIsourceOpen containing any value then
        the ISM_RESOURCE_ELEMENT must have either @ism:FGIsourceOpen or @ism:FGIsourceProtected with a value.
        
        Human Readable: USA documents having FGI Open data must have FGI Open or FGI Protected at
        the resource level. 
    </svrl:text>
            <svrl:text>
        If IC Markings System Register and Manual marking rules do not apply to the document then this
        rule does not apply and this rule returns true. If the current element has attribute @ism:FGIsourceOpen
        specified and does not have attribute @ism:excludeFromRollup set to true, this rule ensures that
        the resourceElement has one of the following attributes specified: @ism:FGIsourceOpen or @ism:FGIsourceProtected.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M276"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00065</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00065</xsl:attribute>
            <svrl:text>
        [ISM-ID-00065][Error] If ISM_USGOV_RESOURCE and any element meeting ISM_CONTRIBUTES in the document 
        have the attribute @ism:FGIsourceProtected containing any value then the ISM_RESOURCE_ELEMENT 
        must have @ism:FGIsourceProtected with a value.
        
        Human Readable: USA documents having FGI Protected data must have FGI Protected at the resource level.
    </svrl:text>
            <svrl:text>
        If IC Markings System Register and Manual rules do not apply to the document then the rule does not apply
        and this rule returns true. If any element has attribute @ism:FGIsourceProtected specified 
        with a non-empty value and does not have attribute @ism:excludeFromRollup set to true, 
        then this rule ensures that the banner has attribute @ism:FGIsourceProtected specified with 
        a non-empty value.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M277"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00133</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00133</xsl:attribute>
            <svrl:text>
		[ISM-ID-00133][Error] If ISM_NSI_EO_APPLIES and attribute 
		@ism:declassException is specified and contains the tokens [25X1-EO-12951],
		[50X1-HUM], [50X2-WMD], [NATO], [AEA] or [NATO-AEA] 
		then attribute @ism:declassDate or @ism:declassEvent must NOT be specified.
		
		Human Readable: Documents under E.O. 13526 must not specify declassDate or declassEvent if 
		a declassException of 25X1-EO-12951, 50X1-HUM, 50X2-WMD, NATO, AEA or NATO-AEA is specified.
	</svrl:text>
            <svrl:text>
		If ISM_NSI_EO_APPLIES, for each element which specifies 
		@ism:declassException with a value containing token [25X1-EO-12951], [50X1-HUM], [50X2-WMD], [NATO], [AEA] 
		or [NATO-AEA] this rule ensures that attributes @ism:declassDate and @ism:declassEvent are NOT specified.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M312"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00141</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00141</xsl:attribute>
            <svrl:text>
        [ISM-ID-00141][Error] If ISM_NSI_EO_APPLIES and:
        1. ISM_RESOURCE_ELEMENT attribute @ism:declassException does not have a value of [25X1-EO-12951], 
        [50X1-HUM], [50X2-WMD], [AEA], [NATO], or [NATO-AEA]
          AND 
        2. ISM_RESOURCE_ELEMENT attribute @ism:declassDate is not specified 
          AND 
        3. ISM_RESOURCE_ELEMENT attribute @ism:declassEvent is not specified 
        
        Human Readable: Documents under E.O. 13526 require declassDate or declassEvent unless 25X1-EO-12951, 
        50X1-HUM, 50X2-WMD, AEA, NATO, or NATO-AEA is specified. 
    </svrl:text>
            <svrl:text>
        If ISM_NSI_EO_APPLIES, the current element is the ISM_RESOURCE_ELEMENT,
        and attribtue @ism:declassExeption is not specified with a value containing the token
        [25X1-EO-12951], [50X1-HUM], or [50X2-WMD], [AEA], [NATO], or [NATO-AEA] then this rule
        ensures that attribute @ism:declassDate is specified or attribute @ism:declassEvent is
        specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M318"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00142</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00142</xsl:attribute>
            <svrl:text>
        [ISM-ID-00142][Error] If the Classified National Security Information
        Executive Order applies to the document, then a classification authority must be
        specified.
    </svrl:text>
            <svrl:text>
        If ISM_NSI_EO_APPLIES is true (defined in ISM_XML.sch), then the
        resource element (has the attribute @ism:resourceElement="true") must have either
        @ism:classifiedBy or @ism:derivativelyClassifiedBy
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M319"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00143</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00143</xsl:attribute>
            <svrl:text>
        [ISM-ID-00143][Error] If ISM_USGOV_RESOURCE and attribute @ism:derivativelyClassifiedBy is specified, 
        then attribute @ism:derivedFrom must be specified. 
        
        Human Readable: Derivatively Classified data including DOE data requires
        a derived from value to be identified.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which 
    	specifies attribute @ism:derivativelyClassifiedBy this rule ensures that
    	attribute @ism:derivedFrom is specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M320"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00168</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00168</xsl:attribute>
            <svrl:text>
        [ISM-ID-00168][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls is not specified or is specified and does not contain the name token 
        [DISPLAYONLY], then attribute @ism:displayOnlyTo must not be specified.
        
        Human Readable: If a portion in a USA document is not marked for DISPLAY ONLY dissemination, 
        it must not list countries to which it may be disclosed. 
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE and attribute @ism:disseminationControls
        does not contain the token [DISPLAYONLY], this rule ensures that the attribute 
      	@ism:displayOnlyTo is not specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M335"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00176</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00176</xsl:attribute>
            <svrl:text>
        [ISM-ID-00176][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:atomicEnergyMarkings has a name token containing [RD] or [FRD], 
        then attributes @ism:declassDate and @ism:declassEvent cannot be specified
        on the resourceElement.

        Human Readable: Automatic declassification of documents containing 
        RD or FRD information is prohibited. Attributes declassDate and 
        declassEvent cannot be used in the classification authority block when 
        RD or FRD is present.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which 
    	has attribute ism:atomicEnergyMarkings specified with a value containing
        the token [RD] or [FRD], this rule ensures that the resourceElement does not
    	have attributes ism:declassDate or ism:declassEvent specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M341"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00213</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00213</xsl:attribute>
            <svrl:text>
        [ISM-ID-00213][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls contains the name token [DISPLAYONLY], then 
        attribute @ism:displayOnlyTo must be specified.
        
        Human Readable: A USA document with DISPLAY ONLY dissemination must 
        indicate the countries to which it may be disclosed.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [DISPLAYONLY] this rule ensures that attribute @ism:displayOnlyTo
    	is specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M370"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00221</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00221</xsl:attribute>
            <svrl:text>
        [ISM-ID-00221][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:derivativelyClassifiedBy is specified, then attributes @ism:classificationReason
        or @ism:classifiedBy must not be specified.
        
        Human Readable: USA documents that are derivatively classified must not
        specify a classification reason or classified by.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which 
    	specifies attribute @ism:derivativelyClassifiedBy this rule ensures that
    	attribute @ism:classificationReason or @ism:classifiedBy is NOT specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M374"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00226</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00226</xsl:attribute>
            <svrl:text>
        [ISM-ID-00226][Error] Attributes @ism:noticeType and @ism:unregisteredNoticeType
        may not both be used on the same element. 
        
        Human Readable: Ensure that the ISM attributes noticeType and
        unregisteredNoticeType are not used on the same element.
    </svrl:text>
            <svrl:text>
        For each element which has attribute ism:noticeType specified, this rule ensures that ism:unregisteredNoticeType
        is not specified. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M376"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00250</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00250</xsl:attribute>
            <svrl:text>
		[ISM-ID-00250][Error] If ISM_USGOV_RESOURCE, element ism:Notice must specify 
		attribute @ism:noticeType or @ism:unregisteredNoticeType.
		
		Human Readable: Notices must specify their type.
	</svrl:text>
            <svrl:text>
		This rule ensures for element ism:Notice must specify their type.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M386"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00299</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00299</xsl:attribute>
            <svrl:text>
        [ISM-ID-00299][Error] If an element contains the attribute @ism:declassException with a value of [AEA], 
        it must also contain the attribute @ism:atomicEnergyMarkings.
    </svrl:text>
            <svrl:text>
		If an element contains an @ism:declassException attribute with a value containing
		[AEA], this rule checks to make sure that element also has an @ism:atomicEnergyMarkings
		attribute.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M434"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00324</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00324</xsl:attribute>
            <svrl:text>
        [ISM-ID-00324][Error] If a document is ISM_USGOV_RESOURCE, it must contain portion markings. 
        
        Human Readable: All valid ISM_USGOV_RESOURCE documents must also contain portion markings. 
    </svrl:text>
            <svrl:text>
        Make sure that all ISM_USGOV_RESOURCE documents contain at least
        one portion mark if they are not uncaveated UNCLASSIFIED. 
        Allow compilation reason to suffice as an exemption from this rule.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M446"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00326</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00326</xsl:attribute>
            <svrl:text>
      [ISM-ID-00326][Error] ORCON information (i.e. @ism:disseminationControls of the resource node contains [OC]) 
      requires ORCON profile NTK metadata.
   </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE and the resource node's @ism:disseminationControls
      attribute contains [OC], the document must have OC profile NTK metadata. That is, there must be an NTK assertion
      with an ntk:AccessPolicy value of ‘urn:us:gov:ic:aces:ntk:oc’.
   </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M448"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00328</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00328</xsl:attribute>
            <svrl:text>
        [ISM-ID-00328][Error] If ISM_USGOV_RESOURCE and: 
        1. Any element in the document that has the attribute @ism:disseminationControls containing [FOUO]
        AND
        2. Has the attribute @ism:classification [U]
        
        Then the element can't have any @ism:nonICMarkings.
        
        Human Readable: Non-IC dissemination control markings in elements of USA Unclassified documents 
        supersede and take precedence over FOUO.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for any element that contains @ism:disseminationControls
        with a value containing [FOUO] and has @ism:classification with a value of [U], 
        then this rule ensures that there is no @ism:nonICMarkings.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M450"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00349</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00349</xsl:attribute>
            <svrl:text>
      [ISM-ID-00349][Error] If ISM_USGOV_RESOURCE, PROPIN information (i.e. @ism:disseminationControls of the resource
      node contains [PR]) requires PROPIN NTK metadata.
   </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE and the resource node's @ism:disseminationControls
      attribute contains [PR], the document must have PROPIN profile NTK metadata. That is, there must be an NTK
      assertion with an ntk:AccessPolicy value that starts with ‘urn:us:gov:ic:aces:ntk:propin:’.
   </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M462"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00350</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00350</xsl:attribute>
            <svrl:text>
      [ISM-ID-00350][Error] Exclusive Distribution information (i.e. @ism:nonICmarkings of the
      resource node contains [XD]) requires XD profile NTK metadata.
   </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE and the resource nodes's @ism:nonICmarkings
      attribute contains [XD], the document must have XD profile NTK metadata. That is, there must be an NTK assertion
      with an ntk:AccessPolicy value of ‘urn:us:gov:ic:aces:ntk:xd’.
   </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M463"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00351</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00351</xsl:attribute>
            <svrl:text>
      [ISM-ID-00351][Error] No Distribution information (i.e. @ism:nonICmarkings of the resource
      node contains [ND]) requires ND profile NTK metadata.
   </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE and the resource node's @ism:nonICmarkings attribute
      contains [ND], the document must have ND profile NTK metadata. That is, there must be an NTK assertion with an
      ntk:AccessPolicy value of ‘urn:us:gov:ic:aces:ntk:nd’.
   </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M464"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00367</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00367</xsl:attribute>
            <svrl:text>
        [ISM-ID-00367][Error] If ISM_USGOV_RESOURCE and attribute @ism:derivedFrom is 
        specified, then attribute @ism:classifiedBy must not be specified.
        
        Human Readable: USA documents that specify a derivative classifier must not also 
        include information related to Original Classification Authorities (classificationReason and classifiedBy).
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE, for each element which 
    	specifies attribute @ism:derivativelyClassifiedBy this rule ensures that
    	attribute @ism:classificationReason or @ism:classifiedBy is NOT specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M476"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00385</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00385</xsl:attribute>
            <svrl:text>
        [ISM-ID-00385][Error] Attribute @ism:declassEvent requires use of attribute @ism:declassDate. 
    </svrl:text>
            <svrl:text>
        CFR policies require that @ism:declassDate accompany @ism:declassEvent. Set context to any element 
        containing @ism:declassEvent attribute. Test if that element also has @ism:declassDate.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M487"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00476</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00476</xsl:attribute>
            <svrl:text> [ISM-ID-00476][Error] If @ism:compliesWith="USA-CUI-ONLY" then attributes
        @ism:SCIcontrols, @ism:SARIdentifier, @ism:atomicEnergyMarkings, @ism:FGIsourceOpen and
        @ism:FGIsourceProtected must not be specified. </svrl:text>
            <svrl:text> If the document has @ism:compliesWith="USA-CUI-ONLY", as defined in
        variable ISM_USCUIONLY_RESOURCE, this rule ensures that NONE of the following attributes are
        specified: @ism:SCIcontrols, @ism:SARIdentifier, @ism:atomicEnergyMarkings,
        @ism:FGIsourceOpen and @ism:FGIsourceProtected . </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M520"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00486</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00486</xsl:attribute>
            <svrl:text>
        [ISM-ID-00486][Error] If ISM_USCUIONLY_RESOURCE or ISM_USCUI_RESOURCE then attribute @ism:nonICmarkings must not be specified.
    </svrl:text>
            <svrl:text>
        If the document is ISM_USCUIONLY_RESOURCE or ISM_USCUI_RESOURCE, this rule ensures that @ism:nonICmarkings 
        does not appear in the document.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M529"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00494</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00494</xsl:attribute>
            <svrl:text>
      [ISM-ID-00494][Error] If ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, then if
      the document contains a PROPIN CUI Category marking (either Basic or Specified), then the
      document MUST have PROPIN_NTK metadata.
      
      Human Readable: PROPIN CUI information (either @ism:cuiBasic or
      @ism:cuiSpecified contains 'PROPIN') requires PROPIN NTK metadata.
   </svrl:text>
            <svrl:text>
      If the document is an ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, and the
      resource node's @ism:cuiBasic or @ism:cuiSpecified attribute contains [PROPIN], then the document must
      have PROPIN NTK profile metadata. That is, there must be an NTK assertion with an
      ntk:AccessPolicy value that starts with ‘urn:us:gov:ic:aces:ntk:propin:’.
   </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M534"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00495</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00495</xsl:attribute>
            <svrl:text> [ISM-ID-00495][Error] If @ism:compliesWith="USA-CUI-ONLY" then attributes
        @ism:classification and @ism:ownerProducer must not be specified. </svrl:text>
            <svrl:text> If the document has @ism:compliesWith="USA-CUI-ONLY", as defined in
        variable ISM_USCUIONLY_RESOURCE, this rule ensures that NONE of the following attributes are
        specified: @ism:classification and @ism:ownerProducer. </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M535"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00497</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00497</xsl:attribute>
            <svrl:text>
        [ISM-ID-00497][Error] If a document contains either @ism:cuiBasic or @ism:cuiSpecified, 
        then the document must contain @ism:cuiControlledBy.
    </svrl:text>
            <svrl:text>
        If a document contains one or both of @ism:cuiBasic or @ism:cuiSpecified on the resource element, 
        this rule ensures that the document contains @ism:cuiControlledBy.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M537"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00499</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00499</xsl:attribute>
            <svrl:text>
        [ISM-ID-00499][Error] If a document has @ism:complieswith="USA-CUI" or "USA-CUI-ONLY", 
        then it must contain @ism:cuiControlledBy.
    </svrl:text>
            <svrl:text>
        If a document has @ism:complieswith="USA-CUI" or "USA-CUI-ONLY", then it must contain @ism:cuiControlledBy.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M539"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00512</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00512</xsl:attribute>
            <svrl:text>
        [ISM-ID-00512][Error] If ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, and attribute 
        @ism:secondBannerLine contains the name token [HVCO], then attribute @ism:handleViaChannels must be specified.
        
        Human Readable: USA documents containing Handle Via Channels Only in the second banner line
        must specify to which channels the document is restricted.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, for each element which has 
        attribute @ism:secondBannerLine specified with a value containing
        the token [HVCO] this rule ensures that attribute @ism:handleViaChannels is specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M548"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00513</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00513</xsl:attribute>
            <svrl:text>
        [ISM-ID-00513][Error] If ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, and attribute 
        @ism:handleViaChannels is specified, then @ism:secondBannerLine MUST contain the name token [HVCO].
        
        Human Readable: USA documents that specify Handle Via Channels MUST specify [HVCO] in the @ism:secondBannerLine attribute.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, for each element which has 
        attribute @ism:handleViaChannels, the element MUST have @ism:secondBannerLine specified with a value containing
        the token [HVCO].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M549"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00518</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00518</xsl:attribute>
            <svrl:text>
        [ISM-ID-00518][Error] For ism:Notice or ism:NoticeExternal, if @ism:noticeProseID is present then ism:NoticeText is prohibited.
    </svrl:text>
            <svrl:text>
        This rule ensures that for ism:Notice or ism:NoticeExternal, if @ism:noticeProseID is present then ism:NoticeText is prohibited.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M554"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00519</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00519</xsl:attribute>
            <svrl:text>
        [ISM-ID-00519][Error] For ism:Notice or ism:NoticeExternal, if @ism:noticeProseID is absent then ism:NoticeText is required.
    </svrl:text>
            <svrl:text>
        This rule ensures that for ism:Notice or ism:NoticeExternal, if @ism:noticeProseID is absent then ism:NoticeText is required.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M555"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00522</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00522</xsl:attribute>
            <svrl:text> [ISM-ID-00522][Error] If ISM_NSI_EO_APPLIES and the @ism:highWaterNATO
        attribute exists on a banner or portion, then @ism:FGIsourceOpen contains [NATO] or
        @ism:ownerProducer contains [NATO] or @ism:FGIsourceProtected contains [FGI]. Human
        Readable: For documents under E.O. 13526, the NATO high-water indicator can only exist on an
        element where either @ism:FGIsourceOpen contains [NATO] or @ism:ownerProducer contains
        [NATO] or @ism:FGIsourceProtected contains [FGI]. </svrl:text>
            <svrl:text> If ISM_NSI_EO_APPLIES, then for each element which specifies attribute
        @ism:highWaterNATO, this rule ensures that at least one of the attributes @ism:ownerProducer
        or @ism:FGIsourceOpen is specified with a value of [NATO] or @ism:FGIsourceProtected is
        specified with a value of [FGI]. </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M557"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00523</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00523</xsl:attribute>
            <svrl:text> [ISM-ID-00523][Error] If ISM_NSI_EO_APPLIES and the @ism:FGIsourceOpen
        attribute contains [NATO] on a banner or portion, then a requirement exists that @ism:highWaterNATO
        also exists, otherwise the NATO data classification cannot be determined. Human Readable: For documents
        under E.O. 13526, if @ism:FGIsourceOpen contains [NATO], then @ism:highWaterNATO must exist. </svrl:text>
            <svrl:text> If ISM_NSI_EO_APPLIES, then the attribute @ism:highWaterNATO must exist when
        @ism:FGIsourceOpen contains [NATO]. </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M558"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00524</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00524</xsl:attribute>
            <svrl:text> [ISM-ID-00524][Error] If ISM_NSI_EO_APPLIES and the @ism:highWaterNATO
        attribute exists on a banner or portion, then @ism:ownerProducer cannot be equal to 'NATO'.
        Human Readable: For documents under E.O. 13526, the NATO high-water indicator is not allowed
        where @ism:ownerProducer='NATO'. It is okay if @ism:ownerProducer contains 'NATO' and other
        tokens like 'USA'. </svrl:text>
            <svrl:text> If ISM_NSI_EO_APPLIES, then for each element which specifies attribute
        @ism:highWaterNATO, this rule produces an error if @ism:ownerProducer='NATO'. </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M559"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00525</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00525</xsl:attribute>
            <svrl:text> [ISM-ID-00525][Error] If ISM_NSI_EO_APPLIES and the @ism:highWaterNATO
        attribute exists on a banner or portion, then @ism:highWaterNATO cannot be higher than @ism:classification.
        Human Readable: For documents under E.O. 13526, the NATO high-water indicator value cannot be
        higher than the classification value.</svrl:text>
            <svrl:text> If ISM_NSI_EO_APPLIES, then for each element which specifies attribute
        @ism:highWaterNATO, this rule checks the value of @ism:classification.  If the value of @ism:highWaterNATO
        is 'NATO-TS' then @ism:classification must be 'TS'. If the value of @ism:highWaterNATO is 'NATO-S',
        then @ism:classification must be 'TS' or 'S'. If the value of @ism:highWaterNATO is 'NATO-C', then
        @ism:classification must be 'C', 'S' or 'TS'.  If the value of @ism:highWaterNATO is 'NATO-R' then
        @ism:classification cannot be 'U'.  If the value of @ism:highWaterNATO is 'NATO-U' then any value
        of classification is ok.  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M560"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00526</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00526</xsl:attribute>
            <svrl:text> [ISM-ID-00526][Error] If ISM_NSI_EO_APPLIES and the @ism:ownerProducer
        attribute contains multiple values on a banner or portion, one being NATO, then a requirement exists that @ism:highWaterNATO
        also exists, otherwise the NATO data classification cannot be determined. Human Readable: For documents
        under E.O. 13526, if @ism:ownerProducer attribute contains multiple values and NATO, then @ism:highWaterNATO must exist. </svrl:text>
            <svrl:text> If ISM_NSI_EO_APPLIES, then the attribute @ism:highWaterNATO must exist when
        @ism:ownerProducer attribute contains multiple values and NATO. </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M561"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00529</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00529</xsl:attribute>
            <svrl:text>
	  	[ISM-ID-00529][Error] All tokens in the @ism:SARIdentifier attribute MUST conform to the regex 
	  	^SAR-[A-Z]{3,}:((C|S|TS):){0,1}[A-Za-z0-9._-]{1,}$ . Human Readable:  All tokens in @ism:SARIdentifier must conform to
	  	a regular expression for: SAR-SourceAuthority:Classification:SAPmarking or SAR-SourceAuthority:SAPmarking.
	</svrl:text>
            <svrl:text>
	  	For all tokens within an @ism:SARIdentifier attribute, this rule ensures that each token follows the regex
	  	^SAR-[A-Z]{3,}:((C|S|TS):){0,1}[A-Za-z0-9._-]{1,}$
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M563"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00531</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00531</xsl:attribute>
            <svrl:text>
	  	[ISM-ID-00531][Error] All resource elements with SAR markings that contain @ism:compliesWith="USGov USDOD USIC" MUST contain 
	  	only one token in @ism:SARIdentifier.  
	</svrl:text>
            <svrl:text>
	  	If there are multiple SARs and if ism:compliesWith contains both tokens  [USIC] and [USDOD], then ERROR.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M565"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00532</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00532</xsl:attribute>
            <svrl:text> [ISM-ID-00532][Error] For all elements with @ism:SARIdentifier with tokens
        that include classification portion marks (e.g., DOD:TS:aaaa or DOD:C:bbbb), the value of
        the classification portion mark cannot be higher than @ism:classification on the same
        element. Human Readable: For @ism:SARIdentifier tokens that include classification portion
        marks in their values, the classification portion mark cannot be higher than the
        classification value. Note that some @ism:SARIdentifier tokens may not contain
        classification portion marks, e.g., DNI:kkkk; the rule does not apply to these tokens. </svrl:text>
            <svrl:text> For all elements with @ism:SARIdentifier with tokens that include
        classification portion marks (e.g., DOD:TS:aaaa or DOD:C:bbbb), check the value of the
        classification portion mark, which is found between two colons ':' according to the regex
        for SARs. The logic uses the fact that if, for example, ':TS:' is found anywhere in
        @ism:SARIdentifier, then the classification of the element should be 'TS'. The rule logic is
        as follows. If there is ':TS:' the @ism:SARIdentifier, then @ism:classification must be
        'TS'. Otherwise, if there is ':S:' in the @ism:SARIdentifier, then @ism:classification must
        be 'S' or 'TS'. Otherwise, if there is ':C:' in the @ism:SARIdentifier, then
        @ism:classification must be 'C' or 'S' or 'TS'. Otherwise, according to the regex, there
        is no classification portion marking in any of the tokens in @ism:SARIdentifier, so do not check against @ism:classification. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M566"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00533</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00533</xsl:attribute>
            <svrl:text>
        [ISM-ID-00533][Error] All resource elements with three or more @ism:SARIdentifier tokens will result in an error when @ism:compliesWith are 
        both DoD and IC.
    </svrl:text>
            <svrl:text>
        If there are 3 or more SARs in the resource node and if ism:compliesWith contains both tokens [USIC] and [USDOD], then ERROR.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M567"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00534</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00534</xsl:attribute>
            <svrl:text>
        [ISM-ID-00534][Error] All elements with @ism:SARIdentifier token(s) containing a dash (-) (excluding the SAR- prefix) will result in an error when @ism:compliesWith are 
        both DoD and IC.  DoD and IC rules differ on how to render SAP markings containing dashes; therefore, it is not allowed to have SAPs 
        with dashes in a document that complies with both DoD and IC rules.
    </svrl:text>
            <svrl:text>
        Find elements with @ism:SARIdentifier when @ism:compliesWith contains both 'USDOD' and 'USIC' ($ISM_USDOD_RESOURCE and $ISM_USIC_RESOURCE).
        If there is any dash in @ism:SARIdentifier after the SAR- prefix, then ERROR.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M568"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00535</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00535</xsl:attribute>
            <svrl:text>
        [ISM-ID-00535][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls contains the name token [WAIVED], then 
        attribute @ism:compliesWith must contain [USDOD]. 
        Human Readable: USA documents containing the WAIVED dissemination control must comply with USDOD rules.
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [WAIVED], this rule ensures that attribute @ism:compliesWith contains [USDOD].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M569"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00012</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00012</xsl:attribute>
            <svrl:text> 
        [ISM-ID-00012][Error] If the document is not USA-CUI-ONLY, AND: 
        1. any of the attributes defined in this DES other than @ism:DESVersion, @ism:ISMCATCESVersion,
        @ism:unregisteredNoticeType, or @ism:pocType are specified for an element, 
        OR
        2. the current node is one of elements arh:Security, arh:ExternalSecurity, ntk:Access or ntk:AccessProfile,
        then attributes @ism:classification and @ism:ownerProducer must be specified for the element.</svrl:text>
            <svrl:text> If the document does NOT have @ism:compliesWith="USA-CUI-ONLY", then for
        each element which defines an attribute in the ISM namespace other than @ism:pocType,
        @ism:DESVersion, @ism:ISMCATCESVersion, or @ism:unregisteredNoticeType, or the element is arh:Security,
        or arh:ExternalSecurity or ntk:Access or ntk:AccessProfile, this rule ensures that
        attributes @ism:classification and @ism:ownerProducer are specified. </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M574"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00102</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00102</xsl:attribute>
            <svrl:text>
        [ISM-ID-00102][Error] The attribute @ism:DESVersion in the namespace urn:us:gov:ic:ism must be specified.   
        
        Human Readable: The data encoding specification version must be specified.
    </svrl:text>
            <svrl:text>
        Make sure that the attribute @ism:DESVersion is specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M575"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00118</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00118</xsl:attribute>
            <svrl:text>
        [ISM-ID-00118][Error] The first element in document order having @ism:resourceElement specified with a value of [true] 
        must have @ism:createDate specified.
    </svrl:text>
            <svrl:text>
        This rule ensures that the resourceElement has attribute @ism:createDate specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M577"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00337</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00337</xsl:attribute>
            <svrl:text>
        [ISM-ID-00337][Error] The first element in document order having @ism:resourceElement specified with a value of [true] 
        must have @ism:compliesWith specified.
    </svrl:text>
            <svrl:text>
        This rule ensures that the resourceElement has attribute @ism:compliesWith specified.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M586"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00449</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00449</xsl:attribute>
            <svrl:text>
        [ISM-ID-00449][Error] The ARH elements cannot be used as root elements.
        
        Human Readable: ARH is not designed to stand-alone and therefore should never
        be used as a root element.
    </svrl:text>
            <svrl:text>
        Ensure that arh:Security or arh:ExternalSecurity are not used as the root element.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M604"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00450</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00450</xsl:attribute>
            <svrl:text>
        [ISM-ID-00450][Warning] @arh:DESVersion is a DEPRECATED attribute.
        
        Human Readable: ARH DESVersion is a DEPRECATED attribute.
    </svrl:text>
            <svrl:text>
        If @arh:DESVersion exists, provide a warning that it is a DEPRECATED attribute.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M605"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00452</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00452</xsl:attribute>
            <svrl:text>
        [ISM-ID-00452][Warning] @ntk:DESVersion is a DEPRECATED attribute.
        
        Human Readable: NTK DESVersion is a DEPRECATED attribute.
    </svrl:text>
            <svrl:text>
        If @ntk:DESVersion exists, provide a warning that it is a DEPRECATED attribute.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M607"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00510</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00510</xsl:attribute>
            <svrl:text>
        [ISM-ID-00510][Error] arh:Security element must contain @ism:resourceElement attribute. 
        
        Human Readable: arh:Security element must contain @ism:resourceElement attribute.
    </svrl:text>
            <svrl:text>
        Find each instance of arh:Security in the document, test that it has @ism:resourceElement.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M609"/>
      </svrl:schematron-output>
   </xsl:template>

   <!--SCHEMATRON PATTERNS-->
<xsl:param name="countriesList"
              select="document('../../CVE/ISMCAT/CVEnumISMCATOwnerProducer.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="classificationAllList"
              select="document('../../CVE/ISM/CVEnumISMClassificationAll.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="classificationUSList"
              select="document('../../CVE/ISM/CVEnumISMClassificationUS.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="ownerProducerList"
              select="document('../../CVE/ISMCAT/CVEnumISMCATOwnerProducer.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="declassExceptionList"
              select="document('../../CVE/ISM/CVEnumISM25X.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="FGIsourceOpenList"
              select="document('../../CVE/ISMCAT/CVEnumISMCATFGIOpen.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="FGIsourceProtectedList"
              select="document('../../CVE/ISMCAT/CVEnumISMCATFGIProtected.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="nonICmarkingsList"
              select="document('../../CVE/ISM/CVEnumISMNonIC.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="releasableToList"
              select="document('../../CVE/ISMCAT/CVEnumISMCATRelTo.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="SCIcontrolsList"
              select="document('../../CVE/ISM/CVEnumISMSCIControls.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="SARIdentifierList"
              select="document('../../CVE/ISM/CVEnumISMSAR.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="SARSourceAuthorityList"
              select="document('../../CVE/ISM/CVEnumISMSARAuthorities.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="validAttributeList"
              select="document('../../CVE/ISM/CVEnumISMAttributes.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="validElementList"
              select="document('../../CVE/ISM/CVEnumISMElements.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="noticeList"
              select="document('../../CVE/ISM/CVEnumISMNotice.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="nonUSControlsList"
              select="document('../../CVE/ISM/CVEnumISMNonUSControls.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="exemptFromList"
              select="document('../../CVE/ISM/CVEnumISMExemptFrom.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="atomicEnergyMarkingsList"
              select="document('../../CVE/ISM/CVEnumISMAtomicEnergyMarkings.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="cuiBasicList"
              select="document('../../CVE/ISM/CVEnumISMCUIBasic.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="cuiSpecifiedList"
              select="document('../../CVE/ISM/CVEnumISMCUISpecified.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="secondBannerLineList"
              select="document('../../CVE/ISM/CVEnumISMSecondBannerLine.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="displayOnlyToList"
              select="document('../../CVE/ISMCAT/CVEnumISMCATRelTo.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="pocTypeList"
              select="document('../../CVE/ISM/CVEnumISMPocType.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="compliesWithList"
              select="document('../../CVE/ISM/CVEnumISMCompliesWith.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="accessPolicyList"
              select="document('../../CVE/ISM/CVEnumNTKAccessPolicy.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="profileDESList"
              select="document('../../CVE/ISM/CVEnumNTKProfileDes.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="licenseList"
              select="document('../../CVE/LIC/CVEnumLicLicense.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="usagencyList"
              select="document('../../CVE/USAgency/CVEnumUSAgencyAcronym.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="issueList"
              select="document('../../CVE/MN/CVEnumMNIssue.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="regionList"
              select="document('../../CVE/MN/CVEnumMNRegion.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="authcatList"
              select="document('../../CVE/AUTHCAT/CVEnumAuthCatType.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="entRoleValueList"
              select="document('../../CVE/ROLE/CVEnumROLEEnterpriseRole.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="NameStartCharPattern" select="':|[A-Z]|_|[a-z]'"/>
   <xsl:param name="NameCharPattern"
              select="concat($NameStartCharPattern, '|-|\.|[0-9]')"/>
   <xsl:param name="NmTokenPattern" select="concat('(', $NameCharPattern, ')+')"/>
   <xsl:param name="NmTokensPattern"
              select="concat($NmTokenPattern, '( ', $NmTokenPattern, ')*')"/>
   <xsl:param name="BooleanPattern" select="'(false|true|0|1)'"/>
   <xsl:param name="DatePattern"
              select="'-?([1-9][0-9]{3,}|0[0-9]{3})-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])(Z|(\+|-)((0[0-9]|1[0-3]):[0-5][0-9]|14:00))?'"/>
   <xsl:param name="catRaw"
              select="document('../../Taxonomy/ISMCAT/TetragraphTaxonomy.xml')"/>
   <xsl:param name="catt"
              select="document('../../Taxonomy/ISMCAT/TetragraphTaxonomyDenormalized.xml')"/>
   <xsl:param name="cattMappings" select="$catt//catt:Tetragraph"/>
   <xsl:param name="tetragraphList"
              select="document('../../CVE/ISMCAT/CVEnumISMCATTetragraph.xml')//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
   <xsl:param name="countriesAndTetras"
              select="          distinct-values(for $each in distinct-values((/descendant-or-self::node()//@ism:ownerProducer | /descendant-or-self::node()//@ism:releasableTo | /descendant-or-self::node()//@ism:displayOnlyTo | /descendant-or-self::node()//@ism:FGIsourceOpen | /descendant-or-self::node()//@ism:FGIsourceProtected))          return             util:tokenize($each))"/>
   <xsl:param name="tetras"
              select="          for $value in $countriesAndTetras          return             if ($catt//catt:Tetragraph[catt:TetraToken = $value]) then                $value             else                null"/>
   <xsl:param name="catt_new"
              select="          for $node in $catt//*          return             if (local-name($node) = 'Organization') then                'MEM'             else                $node"/>
   <xsl:param name="ISM_RESOURCE_ELEMENT"
              select="          (for $each in (//*)          return             if (if (string($each/@ism:resourceElement) castable as xs:boolean) then                if ($each/@ism:resourceElement = true()) then                   true()                else                   false()             else                false()) then                $each             else                null)[1]"/>
   <xsl:param name="ISM_RESOURCE_CREATE_DATE"
              select="$ISM_RESOURCE_ELEMENT/@ism:createDate"/>
   <xsl:param name="builtins"
              select="(('group:iaaems', 'JWICS:IAAEMS'), ('individual:icpki', 'IC-PKI:DN'), ('individual:cadpki', 'CAD-PKI:DN'), ('individual:acsspki', 'ACSS-PKI:DN'), ('organization:usa-agency', 'urn:us:gov:ic:cvenum:usagency:agencyacronym'), ('datasphere:license', 'urn:us:gov:ic:cvenum:lic:license'), ('datasphere:mn:issue', 'urn:us:gov:ic:cvenum:mn:issue'), ('datasphere:mn:region', 'urn:us:gov:ic:cvenum:mn:region'), ('datasphere:rac', 'urn:us:gov:ic:cvenum:authcat:authcattype', ('role:enterpriseRole', 'urn:us:gov:ic:cvenum:role:enterprise:role')))"/>
   <xsl:param name="builtinVocab"
              select="          for $each in $builtins[position() mod 2 eq 1]          return             $each"/>
   <xsl:param name="builtinVocabSource"
              select="          for $each in $builtins[position() mod 2 eq 0]          return             $each"/>
   <xsl:param name="ISM_USGOV_RESOURCE"
              select="util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:compliesWith, ('USGov'))"/>
   <xsl:param name="ISM_OTHER_AUTH_RESOURCE"
              select="util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:compliesWith, ('OtherAuthority'))"/>
   <xsl:param name="ISM_USIC_RESOURCE"
              select="util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:compliesWith, ('USIC'))"/>
   <xsl:param name="ISM_USDOD_RESOURCE"
              select="util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:compliesWith, ('USDOD'))"/>
   <xsl:param name="ISM_USCUI_RESOURCE"
              select="util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:compliesWith, ('USA-CUI'))"/>
   <xsl:param name="ISM_USCUIONLY_RESOURCE"
              select="util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:compliesWith, ('USA-CUI-ONLY'))"/>
   <xsl:param name="disseminationControlsList" select="util:getDissemControlsList()"/>
   <xsl:param name="ISM_710_FDR_EXEMPT"
              select="index-of(tokenize(normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:exemptFrom)), ' '), 'IC_710_MANDATORY_FDR') &gt; 0 or not($ISM_USIC_RESOURCE)"/>
   <xsl:param name="ISM_DOD_DISTRO_EXEMPT"
              select="index-of(tokenize(normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:exemptFrom)), ' '), 'DOD_DISTRO_STATEMENT') &gt; 0 or not($ISM_USDOD_RESOURCE)"/>
   <xsl:param name="ISM_ORCON_POC_DATE" select="xs:date('2011-03-11')"/>
   <xsl:param name="bannerClassification"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:classification))"/>
   <xsl:param name="bannerDisseminationControls"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:disseminationControls))"/>
   <xsl:param name="bannerDisplayOnlyTo"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:displayOnlyTo))"/>
   <xsl:param name="bannerNonICmarkings"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:nonICmarkings))"/>
   <xsl:param name="bannerFGIsourceOpen"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:FGIsourceOpen))"/>
   <xsl:param name="bannerFGIsourceProtected"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:FGIsourceProtected))"/>
   <xsl:param name="bannerReleasableTo"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:releasableTo))"/>
   <xsl:param name="bannerSCIcontrols"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:SCIcontrols))"/>
   <xsl:param name="bannerNotice"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:noticeType))"/>
   <xsl:param name="bannerSARIdentifier"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:SARIdentifier))"/>
   <xsl:param name="bannerAtomicEnergyMarkings"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings))"/>
   <xsl:param name="bannerCuiBasic"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:cuiBasic))"/>
   <xsl:param name="bannerCuiSpecified"
              select="normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:cuiSpecified))"/>
   <xsl:param name="bannerDisseminationControls_tok"
              select="tokenize(normalize-space(string($bannerDisseminationControls)), ' ')"/>
   <xsl:param name="bannerDisplayOnlyTo_tok"
              select="tokenize(normalize-space(string($bannerDisplayOnlyTo)), ' ')"/>
   <xsl:param name="bannerNonICmarkings_tok"
              select="tokenize(normalize-space(string($bannerNonICmarkings)), ' ')"/>
   <xsl:param name="bannerFGIsourceOpen_tok"
              select="tokenize(normalize-space(string($bannerFGIsourceOpen)), ' ')"/>
   <xsl:param name="bannerFGIsourceProtected_tok"
              select="tokenize(normalize-space(string($bannerFGIsourceProtected)), ' ')"/>
   <xsl:param name="bannerReleasableTo_tok"
              select="tokenize(normalize-space(string($bannerReleasableTo)), ' ')"/>
   <xsl:param name="bannerSCIcontrols_tok"
              select="tokenize(normalize-space(string($bannerSCIcontrols)), ' ')"/>
   <xsl:param name="bannerNotice_tok"
              select="tokenize(normalize-space(string($bannerNotice)), ' ')"/>
   <xsl:param name="bannerSARIdentifier_tok"
              select="tokenize(normalize-space(string($bannerSARIdentifier)), ' ')"/>
   <xsl:param name="bannerAtomicEnergyMarkings_tok"
              select="tokenize(normalize-space(string($bannerAtomicEnergyMarkings)), ' ')"/>
   <xsl:param name="bannerCuiBasic_tok"
              select="tokenize(normalize-space(string($bannerCuiBasic)), ' ')"/>
   <xsl:param name="bannerCuiSpecified_tok"
              select="tokenize(normalize-space(string($bannerCuiSpecified)), ' ')"/>
   <xsl:param name="partTags"
              select="/descendant-or-self::node()[@ism:* except (@ism:pocType | @ism:DESVersion | @ism:unregisteredNoticeType | @ism:ISMCATCESVersion) and util:contributesToRollup(.) and not(generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT))]"/>
   <xsl:param name="partClassification"
              select="          for $token in $partTags/@ism:classification          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partOwnerProducer"
              select="          for $token in $partTags/@ism:ownerProducer          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partDisseminationControls"
              select="          for $token in $partTags/@ism:disseminationControls          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partDisplayOnlyTo"
              select="          for $token in $partTags/@ism:displayOnlyTo          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partAtomicEnergyMarkings"
              select="          for $token in $partTags/@ism:atomicEnergyMarkings          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partNonICmarkings"
              select="          for $token in $partTags/@ism:nonICmarkings          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partFGIsourceOpen"
              select="          for $token in $partTags/@ism:FGIsourceOpen          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partFGIsourceProtected"
              select="          for $token in $partTags/@ism:FGIsourceProtected          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partSCIcontrols"
              select="          for $token in $partTags/@ism:SCIcontrols          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partNoticeType"
              select="          for $token in $partTags/@ism:noticeType          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partSARIdentifier"
              select="          for $token in $partTags/@ism:SARIdentifier          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partCuiBasicTags"
              select="/descendant-or-self::node()[@ism:cuiBasic and util:contributesToRollup(.) and not(generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT))]"/>
   <xsl:param name="partCuiBasic"
              select="          for $token in $partCuiBasicTags/@ism:cuiBasic          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partCuiSpecifiedTags"
              select="/descendant-or-self::node()[@ism:cuiSpecified and util:contributesToRollup(.) and not(generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT))]"/>
   <xsl:param name="partCuiSpecified"
              select="          for $token in $partCuiSpecifiedTags/@ism:cuiSpecified          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partPocType"
              select="//*/@ism:pocType[util:contributesToRollup(./parent::node()) and not(generate-id(./parent::node()) = generate-id($ISM_RESOURCE_ELEMENT)) and not(./parent::node()/@ism:externalNotice = true())]"/>
   <xsl:param name="partClassification_tok"
              select="          for $token in $partClassification          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partOwnerProducer_tok"
              select="          for $token in $partOwnerProducer          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partDisseminationControls_tok"
              select="          for $token in $partDisseminationControls          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partDisplayOnlyTo_tok"
              select="          for $token in $partDisplayOnlyTo          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partAtomicEnergyMarkings_tok"
              select="          for $token in $partAtomicEnergyMarkings          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partNonICmarkings_tok"
              select="          for $token in $partNonICmarkings          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partSCIcontrols_tok"
              select="          for $token in $partSCIcontrols          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partNoticeType_tok"
              select="          for $token in $partNoticeType          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partSARIdentifier_tok"
              select="          for $token in $partSARIdentifier          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partPocType_tok"
              select="          for $token in $partPocType          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partCuiBasic_tok"
              select="          for $token in $partCuiBasic          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partCuiSpecified_tok"
              select="          for $token in $partCuiSpecified          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="partNoticeNodeType"
              select="          for $token in $partTags/@ism:noticeType          return             tokenize(normalize-space(string($token)), ' ')"/>
   <xsl:param name="ISM_NSI_EO_APPLIES"
              select="          $ISM_USGOV_RESOURCE and not($ISM_RESOURCE_ELEMENT/@ism:classification = 'U') and $ISM_RESOURCE_CREATE_DATE &gt;= xs:date('1996-04-11') and (some $element in $partTags             satisfies not($element/@ism:classification = 'U') and not($element/@ism:atomicEnergyMarkings))"/>
   <xsl:param name="dcTags"
              select="          for $piece in $disseminationControlsList          return             $piece/text()"/>
   <xsl:param name="dcTagsFound"
              select="          for $token in $dcTags          return             if (index-of($partDisseminationControls_tok, $token) &gt; 0 and (not(index-of($bannerDisseminationControls_tok, $token) &gt; 0))) then                $token             else                null"/>
   <xsl:param name="aeaTags"
              select="          for $piece in $atomicEnergyMarkingsList          return             $piece/text()"/>
   <xsl:param name="aeaTagsFound"
              select="          for $token in $aeaTags          return             if (index-of($partAtomicEnergyMarkings_tok, $token) &gt; 0 and (not(index-of($bannerAtomicEnergyMarkings_tok, $token) &gt; 0))) then                $token             else                null"/>
   <xsl:param name="ACCMRegex" select="'^ACCM-[A-Z0-9\-_]{1,61}$'"/>
   <xsl:param name="nonACCMLeftSet" select="'DS'"/>
   <xsl:param name="nonACCMRightSet" select="'XD,ND,SBU,SBU-NF,LES,LES-NF,SSI,NNPI'"/>
   <xsl:param name="nonACCMLeftSetTok" select="tokenize($nonACCMLeftSet, ',')"/>
   <xsl:param name="nonACCMRightSetTok" select="tokenize($nonACCMRightSet, ',')"/>
   <xsl:param name="decomposableTetraElems"
              select="$cattMappings[@decomposable[. = 'Yes' or . = 'NA']]"/>
   <xsl:param name="decomposableTetras"
              select="$decomposableTetraElems/catt:TetraToken/text()"/>
   <xsl:param name="countFdrPortions" select="count($partTags[util:containsFDR(.)])"/>
   <xsl:param name="relToCalculatedBannerTokens"
              select="util:calculateCommonCountries($partTags/@ism:releasableTo)"/>
   <xsl:param name="relToActualBannerTokens"
              select="util:expandDecomposableTetras($ISM_RESOURCE_ELEMENT/@ism:releasableTo)"/>
   <xsl:param name="displayToCalculatedBannerTokens"
              select="util:calculateCommonCountries(($partTags/@ism:releasableTo, $partTags/@ism:displayOnlyTo))"/>
   <xsl:param name="displayToActualBannerTokens"
              select="util:expandDecomposableTetras(util:join(($ISM_RESOURCE_ELEMENT/@ism:releasableTo, $ISM_RESOURCE_ELEMENT/@ism:displayOnlyTo)))"/>

   <!--PATTERN ISM-ID-00405-->


	<!--RULE ISM-ID-00405-R1-->
<xsl:template match="ntk:AccessProfile[ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:mn']/ntk:AccessProfileValue"
                 priority="1000"
                 mode="M198">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:AccessProfile[ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:mn']/ntk:AccessProfileValue"
                       id="ISM-ID-00405-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ntk:qualifier)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="not(@ntk:qualifier)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00405][Error] The Access Profile Value must not have an @ntk:qualifier attribute specified
            for MN NTK assertions.</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M198"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M198"/>
   <xsl:template match="@*|node()" priority="-2" mode="M198">
      <xsl:apply-templates select="*" mode="M198"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00406-->


	<!--RULE ISM-ID-00406-R1-->
<xsl:template match="ntk:AccessProfile[ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:mn']/ntk:VocabularyType"
                 priority="1000"
                 mode="M199">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:AccessProfile[ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:mn']/ntk:VocabularyType"
                       id="ISM-ID-00406-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ntk:sourceVersion"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ntk:sourceVersion">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00406][Error] The @ntk:sourceVersion attribute is required.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ntk:name = 'datasphere:mn:issue' or @ntk:name = 'datasphere:mn:region'"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ntk:name = 'datasphere:mn:issue' or @ntk:name = 'datasphere:mn:region'">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00406][Error] The name attribute must be ‘datasphere:mn:issue’ or ‘datasphere:mn:region’.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M199"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M199"/>
   <xsl:template match="@*|node()" priority="-2" mode="M199">
      <xsl:apply-templates select="*" mode="M199"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00408-->


	<!--RULE ISM-ID-00408-R1-->
<xsl:template match="ntk:AccessProfile[ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:propin:2']"
                 priority="1000"
                 mode="M201">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:AccessProfile[ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:propin:2']"
                       id="ISM-ID-00408-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="ntk:ProfileDes"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="ntk:ProfileDes">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00408][Error] NTK assertions that use the ‘urn:us:gov:ic:aces:ntk:propin:2’ access policy 
            MUST specify an ntk:ProfileDes element.</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M201"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M201"/>
   <xsl:template match="@*|node()" priority="-2" mode="M201">
      <xsl:apply-templates select="*" mode="M201"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00416-->


	<!--RULE ISM-ID-00416-R1-->
<xsl:template match="ntk:AccessProfile[ntk:AccessProfileValue or ntk:VocabularyType]"
                 priority="1000"
                 mode="M209">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:AccessProfile[ntk:AccessProfileValue or ntk:VocabularyType]"
                       id="ISM-ID-00416-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="ntk:ProfileDes"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="ntk:ProfileDes">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00416][Error] If ntk:AccessProfileValue or ntk:VocabularyType are specified then there must
            be a Profile DES that defines the use of the ntk:AccessProfile structure.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M209"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M209"/>
   <xsl:template match="@*|node()" priority="-2" mode="M209">
      <xsl:apply-templates select="*" mode="M209"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00417-->


	<!--RULE ISM-ID-00417-R1-->
<xsl:template match="ntk:ProfileDes" priority="1000" mode="M210">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:ProfileDes"
                       id="ISM-ID-00417-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="following-sibling::ntk:AccessProfileValue"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="following-sibling::ntk:AccessProfileValue">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00417][Error] If there is a Profile DES specified, then there must be at least
            one ntk:AccessProfileValue.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M210"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M210"/>
   <xsl:template match="@*|node()" priority="-2" mode="M210">
      <xsl:apply-templates select="*" mode="M210"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00419-->


	<!--RULE ISM-ID-00419-R1-->
<xsl:template match="ntk:AccessProfile[ntk:AccessPolicy='urn:us:gov:ic:aces:ntk:ico']"
                 priority="1000"
                 mode="M212">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:AccessProfile[ntk:AccessPolicy='urn:us:gov:ic:aces:ntk:ico']"
                       id="ISM-ID-00419-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(ntk:ProfileDes | ntk:VocabularyType | ntk:AccessProfileValue)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(ntk:ProfileDes | ntk:VocabularyType | ntk:AccessProfileValue)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00419][Error] ntk:AccessProfile containing the ntk:AccessPolicy [urn:us:gov:ic:aces:ntk:ico] may not have
            ntk:ProfileDes, ntk:VocabularyType, or ntk:AccessProfileValue elements specified.
            
            Human Readable: When the ICO ACES is referenced, no data content may be specified in the ntk:AccessProfile.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M212"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M212"/>
   <xsl:template match="@*|node()" priority="-2" mode="M212">
      <xsl:apply-templates select="*" mode="M212"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00421-->


	<!--RULE ISM-ID-00421-R1-->
<xsl:template match="ntk:AccessProfile[ntk:ProfileDes='urn:us:gov:ic:ntk:profile:agencydissem']"
                 priority="1000"
                 mode="M214">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:AccessProfile[ntk:ProfileDes='urn:us:gov:ic:ntk:profile:agencydissem']"
                       id="ISM-ID-00421-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count(ntk:AccessProfileValue[@ntk:qualifier='originator']) = 1"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count(ntk:AccessProfileValue[@ntk:qualifier='originator']) = 1">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00421][Error] An Agency Dissemination NTK must have one and only one entry
            qualified as the originator.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M214"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M214"/>
   <xsl:template match="@*|node()" priority="-2" mode="M214">
      <xsl:apply-templates select="*" mode="M214"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00422-->


	<!--RULE -->
<xsl:template match="ntk:AccessProfile[ntk:AccessProfileValue/@ntk:vocabulary='organization:usa-agency']"
                 priority="1000"
                 mode="M215">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:AccessProfile[ntk:AccessProfileValue/@ntk:vocabulary='organization:usa-agency']"/>

		    <!--ASSERT -->
<xsl:choose>
         <xsl:when test="ntk:VocabularyType[@ntk:name='organization:usa-agency']/@ntk:sourceVersion"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="ntk:VocabularyType[@ntk:name='organization:usa-agency']/@ntk:sourceVersion">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00422][Error] An @ntk:sourceVersion must be specified for the built-in organization:usa-agency vocabulary type.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M215"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M215"/>
   <xsl:template match="@*|node()" priority="-2" mode="M215">
      <xsl:apply-templates select="*" mode="M215"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00425-->


	<!--RULE -->
<xsl:template match="ntk:AccessProfile[ntk:AccessProfileValue/@ntk:vocabulary='datasphere:mn:issue']"
                 priority="1000"
                 mode="M218">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:AccessProfile[ntk:AccessProfileValue/@ntk:vocabulary='datasphere:mn:issue']"/>

		    <!--ASSERT -->
<xsl:choose>
         <xsl:when test="ntk:VocabularyType[@ntk:name='datasphere:mn:issue']/@ntk:sourceVersion"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="ntk:VocabularyType[@ntk:name='datasphere:mn:issue']/@ntk:sourceVersion">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00425][Error] An @ntk:sourceVersion must be specified for the built-in datasphere:mn:issue vocabulary type.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M218"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M218"/>
   <xsl:template match="@*|node()" priority="-2" mode="M218">
      <xsl:apply-templates select="*" mode="M218"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00426-->


	<!--RULE -->
<xsl:template match="ntk:AccessProfile[ntk:AccessProfileValue/@ntk:vocabulary='datasphere:mn:region']"
                 priority="1000"
                 mode="M219">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:AccessProfile[ntk:AccessProfileValue/@ntk:vocabulary='datasphere:mn:region']"/>

		    <!--ASSERT -->
<xsl:choose>
         <xsl:when test="ntk:VocabularyType[@ntk:name='datasphere:mn:region']/@ntk:sourceVersion"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="ntk:VocabularyType[@ntk:name='datasphere:mn:region']/@ntk:sourceVersion">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00426][Error]An @ntk:sourceVersion must be specified for the built-in datasphere:mn:region vocabulary type.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M219"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M219"/>
   <xsl:template match="@*|node()" priority="-2" mode="M219">
      <xsl:apply-templates select="*" mode="M219"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00437-->


	<!--RULE -->
<xsl:template match="ntk:AccessProfile[ntk:AccessProfileValue/@ntk:vocabulary='datasphere:license']"
                 priority="1000"
                 mode="M230">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:AccessProfile[ntk:AccessProfileValue/@ntk:vocabulary='datasphere:license']"/>

		    <!--ASSERT -->
<xsl:choose>
         <xsl:when test="ntk:VocabularyType[@ntk:name='datasphere:license']/@ntk:sourceVersion"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="ntk:VocabularyType[@ntk:name='datasphere:license']/@ntk:sourceVersion">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00437][Error]An @ntk:sourceVersion must be specified for the built-in datasphere:license vocabulary type.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M230"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M230"/>
   <xsl:template match="@*|node()" priority="-2" mode="M230">
      <xsl:apply-templates select="*" mode="M230"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00454-->


	<!--RULE -->
<xsl:template match="ntk:AccessProfile[ntk:AccessProfileValue/@ntk:vocabulary='datasphere:rac']"
                 priority="1000"
                 mode="M234">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:AccessProfile[ntk:AccessProfileValue/@ntk:vocabulary='datasphere:rac']"/>

		    <!--ASSERT -->
<xsl:choose>
         <xsl:when test="ntk:VocabularyType[@ntk:name='datasphere:rac']/@ntk:sourceVersion"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="ntk:VocabularyType[@ntk:name='datasphere:rac']/@ntk:sourceVersion">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
                  <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00454][Error]An @ntk:sourceVersion must be specified for the built-in datasphere:rac vocabulary type.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M234"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M234"/>
   <xsl:template match="@*|node()" priority="-2" mode="M234">
      <xsl:apply-templates select="*" mode="M234"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00455-->


	<!--RULE ISM-ID-00455-R1-->
<xsl:template match="ntk:RequiresAnyOf|ntk:RequiresAllOf"
                 priority="1000"
                 mode="M235">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ntk:RequiresAnyOf|ntk:RequiresAllOf"
                       id="ISM-ID-00455-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="ntk:AccessProfileList"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="ntk:AccessProfileList">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00455][Error] ntk:RequiresAnyOf and ntk:RequiresAllOf must contain ntk:AccessProfileList.            
            
            Human Readable: ntk:RequiresAnyOf and ntk:RequiresAllOf must have the child element ntk:AccessProfileList.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M235"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M235"/>
   <xsl:template match="@*|node()" priority="-2" mode="M235">
      <xsl:apply-templates select="*" mode="M235"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00157-->


	<!--RULE ISM-ID-00157-R1-->
<xsl:template match="*[$ISM_USDOD_RESOURCE and util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-B', 'DoD-Dist-C', 'DoD-Dist-D', 'DoD-Dist-E'))]"
                 priority="1000"
                 mode="M246">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USDOD_RESOURCE and util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-B', 'DoD-Dist-C', 'DoD-Dist-D', 'DoD-Dist-E'))]"
                       id="ISM-ID-00157-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:noticeReason"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:noticeReason">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00157][Error] If ISM_USDOD_RESOURCE and: 
            1. The attribute notice contains one of the [DoD-Dist-B], [DoD-Dist-C], [DoD-Dist-D], or [DoD-Dist-E] 
            AND
            2. The attribute @ism:noticeReason is not specified. 
            
            Human Readable: DoD distribution statements B, C, D , or E all require a reason. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M246"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M246"/>
   <xsl:template match="@*|node()" priority="-2" mode="M246">
      <xsl:apply-templates select="*" mode="M246"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00161-->


	<!--RULE ISM-ID-00161-R1-->
<xsl:template match="*[$ISM_USDOD_RESOURCE and (util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:noticeType, ('DoD-Dist-A'))) and not (@ism:excludeFromRollup=true())]"
                 priority="1000"
                 mode="M248">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USDOD_RESOURCE and (util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:noticeType, ('DoD-Dist-A'))) and not (@ism:excludeFromRollup=true())]"
                       id="ISM-ID-00161-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:nonICmarkings)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="not(@ism:nonICmarkings)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> 
            [ISM-ID-00161][Error] If the document is an
            1. ISM_USDOD_RESOURCE AND
            2. the attribute @ism:noticeType of ISM_RESOURCE_ELEMENT contains [DoD-Dist-A] AND
            3. no portions in the document have their attribute @ism:excludeFromRollup set to [true]
            THEN there must not be any attribute @ism:nonICmarkings present.
            
            Human Readable: Distribution statement A (Public Release) is 
            incompatible with any nonICMarkings if excludeFromRollup is not TRUE.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M248"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M248"/>
   <xsl:template match="@*|node()" priority="-2" mode="M248">
      <xsl:apply-templates select="*" mode="M248"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00237-->


	<!--RULE ISM-ID-00237-R1-->
<xsl:template match="*[$ISM_USDOD_RESOURCE and util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-B', 'DoD-Dist-C', 'DoD-Dist-D', 'DoD-Dist-E', 'DoD-Dist-F'))]"
                 priority="1000"
                 mode="M251">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USDOD_RESOURCE and util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-B', 'DoD-Dist-C', 'DoD-Dist-D', 'DoD-Dist-E', 'DoD-Dist-F'))]"
                       id="ISM-ID-00237-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:noticeDate"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:noticeDate">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> 
            [ISM-ID-00237][Error] If ISM_USDOD_RESOURCE, any element which specifies
            attribute @ism:noticeType containing one of the tokens [DoD-Dist-B], 
            [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], or [DoD-Dist-F]
            must also specify attribute @ism:noticeDate.     	
            
            Human Readable: DoD distribution statements B, C, D, E, and F all require a date.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M251"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M251"/>
   <xsl:template match="@*|node()" priority="-2" mode="M251">
      <xsl:apply-templates select="*" mode="M251"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00239-->


	<!--RULE ISM-ID-00239-R1-->
<xsl:template match="*[$ISM_USDOD_RESOURCE  and util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:noticeType, ('DoD-Dist-A')) and not(@ism:excludeFromRollup=true())]"
                 priority="1000"
                 mode="M253">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USDOD_RESOURCE  and util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:noticeType, ('DoD-Dist-A')) and not(@ism:excludeFromRollup=true())]"
                       id="ISM-ID-00239-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:disseminationControls)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(@ism:disseminationControls)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> 
		    	[ISM-ID-00239][Error] If ISM_USDOD_RESOURCE and attribute @ism:noticeType of
		    	ISM_RESOURCE_ELEMENT contains the token [DoD-Dist-A], then any element 
		    	which contributes to rollup should not have an attribute
		    	@ism:disseminationControls present.
		    	
		    	Human Readable: Distribution statement A (Public Release) is incompatible 
		    	with @ism:disseminationControls present for contributing portions.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M253"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M253"/>
   <xsl:template match="@*|node()" priority="-2" mode="M253">
      <xsl:apply-templates select="*" mode="M253"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00240-->


	<!--RULE ISM-ID-00240-R1-->
<xsl:template match="*[$ISM_USDOD_RESOURCE and util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:noticeType, ('DoD-Dist-A')) and not(@ism:excludeFromRollup=true())]"
                 priority="1000"
                 mode="M254">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USDOD_RESOURCE and util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:noticeType, ('DoD-Dist-A')) and not(@ism:excludeFromRollup=true())]"
                       id="ISM-ID-00240-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:atomicEnergyMarkings)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(@ism:atomicEnergyMarkings)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> 
            [ISM-ID-00240][Error] If ISM_USDOD_RESOURCE and attribute @ism:noticeType of
            ISM_RESOURCE_ELEMENT contains the token [DoD-Dist-A], then any element
            which contributes to rollup should not have an attribute @ism:atomicEnergyMarkings present.
            
            Human Readable: Distribution statement A (Public Release) is incompatible 
            with presence of @ism:atomicEnergyMarkings.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M254"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M254"/>
   <xsl:template match="@*|node()" priority="-2" mode="M254">
      <xsl:apply-templates select="*" mode="M254"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00527-->


	<!--RULE ISM-ID-00527-R1-->
<xsl:template match="*[@ism:resourceElement='true' and @ism:SARIdentifier]"
                 priority="1000"
                 mode="M255">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:resourceElement='true' and @ism:SARIdentifier]"
                       id="ISM-ID-00527-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="if ((some $token in tokenize(normalize-space(string(./@ism:SARIdentifier)),' ')     satisfies starts-with($token,'SAR-DOD:')) and @ism:declassException) then true()    else if ((some $token in tokenize(normalize-space(string(./@ism:SARIdentifier)),' ')     satisfies starts-with($token,'SAR-DOD:')) and not(@ism:declassException)) then false()    else true()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if ((some $token in tokenize(normalize-space(string(./@ism:SARIdentifier)),' ') satisfies starts-with($token,'SAR-DOD:')) and @ism:declassException) then true() else if ((some $token in tokenize(normalize-space(string(./@ism:SARIdentifier)),' ') satisfies starts-with($token,'SAR-DOD:')) and not(@ism:declassException)) then false() else true()">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00527][Warning] All resource elements that contain a DOD @ism:SARIdentifier attribute SHOULD contain attribute
		    	@ism:declassException. Per the OSD Declassification Guide, there is an ISCAP Files Series Exemption (FSE) on 
		    	records within DoD Special Access Programs (SAPs) files. This Exemption functions as a 25X, and therefore the records 
		    	in these files are exempted from automatic declassification for 50 years. This document does not apply any declassification 
		    	exemption; recommend verifying that this is correct.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M255"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M255"/>
   <xsl:template match="@*|node()" priority="-2" mode="M255">
      <xsl:apply-templates select="*" mode="M255"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00014-->


	<!--RULE ISM-ID-00014-R1-->
<xsl:template match="*[$ISM_NSI_EO_APPLIES and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                 priority="1000"
                 mode="M256">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_NSI_EO_APPLIES and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                       id="ISM-ID-00014-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:declassDate or @ism:declassEvent or @ism:declassException"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:declassDate or @ism:declassEvent or @ism:declassException">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00014][Error] If ISM_NSI_EO_APPLIES then one or more of the following 
            attributes: @ism:declassDate, @ism:declassEvent, or @ism:declassException must be specified on the ISM_RESOURCE_ELEMENT.
            Human Readable: Documents under E.O. 13526 must have declassification instructions included in the 
            classification authority block information.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M256"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M256"/>
   <xsl:template match="@*|node()" priority="-2" mode="M256">
      <xsl:apply-templates select="*" mode="M256"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00016-->


	<!--RULE ISM-ID-00016-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and @ism:classification='U']"
                 priority="1000"
                 mode="M257">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and @ism:classification='U']"
                       id="ISM-ID-00016-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:classificationReason or @ism:classifiedBy or @ism:declassDate or @ism:declassEvent or @ism:declassException or @ism:derivativelyClassifiedBy or @ism:derivedFrom or @ism:SARIdentifier or @ism:SCIcontrols)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(@ism:classificationReason or @ism:classifiedBy or @ism:declassDate or @ism:declassEvent or @ism:declassException or @ism:derivativelyClassifiedBy or @ism:derivedFrom or @ism:SARIdentifier or @ism:SCIcontrols)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00016][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:classification has a value of [U], then attributes @ism:classificationReason,
            @ism:classifiedBy, @ism:derivativelyClassifiedBy, @ism:declassDate, @ism:declassEvent, 
            @ism:declassException, @ism:derivedFrom, @ism:SARIdentifier, or @ism:SCIcontrols must not be specified.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M257"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M257"/>
   <xsl:template match="@*|node()" priority="-2" mode="M257">
      <xsl:apply-templates select="*" mode="M257"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00017-->


	<!--RULE ISM-ID-00017-R1-->
<xsl:template match="*[$ISM_NSI_EO_APPLIES and @ism:classifiedBy]"
                 priority="1000"
                 mode="M258">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_NSI_EO_APPLIES and @ism:classifiedBy]"
                       id="ISM-ID-00017-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:classificationReason"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:classificationReason">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00017][Error] If ISM_NSI_EO_APPLIES and attribute 
            @ism:classifiedBy is specified, then attribute @ism:classificationReason must be specified.         
            Human Readable: Documents under E.O. 13526 containing Originally Classified data require a
            classification reason to be identified.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M258"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M258"/>
   <xsl:template match="@*|node()" priority="-2" mode="M258">
      <xsl:apply-templates select="*" mode="M258"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00031-->


	<!--RULE ISM-ID-00031-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL', 'EYES'))]"
                 priority="1000"
                 mode="M261">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL', 'EYES'))]"
                       id="ISM-ID-00031-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:releasableTo"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:releasableTo">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00031][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls contains the name token [REL] or [EYES], then 
            attribute @ism:releasableTo must be specified. 
            Human Readable: USA documents containing REL TO or EYES ONLY 
            dissemination must specify to which countries the document is releasable.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M261"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M261"/>
   <xsl:template match="@*|node()" priority="-2" mode="M261">
      <xsl:apply-templates select="*" mode="M261"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00032-->


	<!--RULE ISM-ID-00032-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL', 'EYES')))]"
                 priority="1000"
                 mode="M262">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL', 'EYES')))]"
                       id="ISM-ID-00032-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:releasableTo)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="not(@ism:releasableTo)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00032][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls is not specified, or is specified and does not 
            contain the name token [REL] or [EYES], then attribute @ism:releasableTo 
            must not be specified.
            
            Human Readable: USA documents must only specify to which countries it is 
            authorized for release if dissemination information contains 
            REL TO or EYES ONLY data. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M262"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M262"/>
   <xsl:template match="@*|node()" priority="-2" mode="M262">
      <xsl:apply-templates select="*" mode="M262"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00064-->


	<!--RULE ISM-ID-00064-R1-->
<xsl:template match="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                 priority="1000"
                 mode="M276">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                       id="ISM-ID-00064-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if(not($ISM_USGOV_RESOURCE)) then true() else if(not(empty($partFGIsourceOpen))) then ($bannerFGIsourceOpen or $bannerFGIsourceProtected) else true()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if(not($ISM_USGOV_RESOURCE)) then true() else if(not(empty($partFGIsourceOpen))) then ($bannerFGIsourceOpen or $bannerFGIsourceProtected) else true()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00064][Error] If ISM_USGOV_RESOURCE and any element meeting
            ISM_CONTRIBUTES in the document have the attribute @ism:FGIsourceOpen containing any value then
            the ISM_RESOURCE_ELEMENT must have either @ism:FGIsourceOpen or @ism:FGIsourceProtected with a value.
            
            Human Readable: USA documents having FGI Open data must have FGI Open or FGI Protected at
            the resource level. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M276"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M276"/>
   <xsl:template match="@*|node()" priority="-2" mode="M276">
      <xsl:apply-templates select="*" mode="M276"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00065-->


	<!--RULE ISM-ID-00065-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(empty($partFGIsourceProtected))]"
                 priority="1000"
                 mode="M277">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(empty($partFGIsourceProtected))]"
                       id="ISM-ID-00065-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:FGIsourceProtected"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:FGIsourceProtected">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00065][Error] If ISM_USGOV_RESOURCE and any element meeting ISM_CONTRIBUTES in the document 
            have the attribute @ism:FGIsourceProtected containing any value then the ISM_RESOURCE_ELEMENT 
            must have @ism:FGIsourceProtected with a value.
            
            Human Readable: USA documents having FGI Protected data must have FGI Protected at the resource level.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M277"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M277"/>
   <xsl:template match="@*|node()" priority="-2" mode="M277">
      <xsl:apply-templates select="*" mode="M277"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00133-->


	<!--RULE ISM-ID-00133-R1-->
<xsl:template match="*[$ISM_NSI_EO_APPLIES and util:containsAnyOfTheTokens(@ism:declassException, ('25X1-EO-12951', '50X1-HUM', '50X2-WMD', 'NATO', 'AEA', 'NATO-AEA'))]"
                 priority="1000"
                 mode="M312">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_NSI_EO_APPLIES and util:containsAnyOfTheTokens(@ism:declassException, ('25X1-EO-12951', '50X1-HUM', '50X2-WMD', 'NATO', 'AEA', 'NATO-AEA'))]"
                       id="ISM-ID-00133-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:declassDate or @ism:declassEvent)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(@ism:declassDate or @ism:declassEvent)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[ISM-ID-00133][Error] If ISM_NSI_EO_APPLIES and attribute 
			@ism:declassException is specified and contains the tokens [25X1-EO-12951],
			[50X1-HUM], [50X2-WMD], [NATO], [AEA] or [NATO-AEA] 
			then attribute @ism:declassDate or @ism:declassEvent must NOT be specified.
			
			Human Readable: Documents under E.O. 13526 must not specify declassDate or declassEvent if 
			a declassException of 25X1-EO-12951, 50X1-HUM, 50X2-WMD, NATO, AEA or NATO-AEA is specified.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M312"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M312"/>
   <xsl:template match="@*|node()" priority="-2" mode="M312">
      <xsl:apply-templates select="*" mode="M312"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00141-->


	<!--RULE ISM-ID-00141-R1-->
<xsl:template match="*[$ISM_NSI_EO_APPLIES and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(util:containsAnyOfTheTokens(@ism:declassException, ('25X1-EO-12951', '50X1-HUM', '50X2-WMD', 'AEA', 'NATO', 'NATO-AEA')))]"
                 priority="1000"
                 mode="M318">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_NSI_EO_APPLIES and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(util:containsAnyOfTheTokens(@ism:declassException, ('25X1-EO-12951', '50X1-HUM', '50X2-WMD', 'AEA', 'NATO', 'NATO-AEA')))]"
                       id="ISM-ID-00141-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:declassDate or @ism:declassEvent"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:declassDate or @ism:declassEvent">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00141][Error] If ISM_NSI_EO_APPLIES and:
            1. ISM_RESOURCE_ELEMENT attribute @ism:declassException does not have a value of [25X1-EO-12951], 
            [50X1-HUM], [50X2-WMD], [AEA], [NATO], or [NATO-AEA]
            AND 
            2. ISM_RESOURCE_ELEMENT attribute @ism:declassDate is not specified 
            AND 
            3. ISM_RESOURCE_ELEMENT attribute @ism:declassEvent is not specified 
            
            Human Readable: Documents under E.O. 13526 require declassDate or declassEvent unless 25X1-EO-12951, 
            50X1-HUM, 50X2-WMD, AEA, NATO, or NATO-AEA is specified. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M318"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M318"/>
   <xsl:template match="@*|node()" priority="-2" mode="M318">
      <xsl:apply-templates select="*" mode="M318"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00142-->


	<!--RULE ISM-ID-00142-R1-->
<xsl:template match="*[@ism:resourceElement='true' and @ism:classification != 'U' and util:containsAnyOfTheTokens(@ism:ownerProducer, ('USA'))]"
                 priority="1000"
                 mode="M319">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:resourceElement='true' and @ism:classification != 'U' and util:containsAnyOfTheTokens(@ism:ownerProducer, ('USA'))]"
                       id="ISM-ID-00142-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:classifiedBy or @ism:derivativelyClassifiedBy"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:classifiedBy or @ism:derivativelyClassifiedBy">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00142][Error] If ISM_NSI_EO_APPLIES is true (defined in ISM_XML.sch), then the
            resource element (has the attribute @ism:resourceElement="true") must have either
            @ism:classifiedBy or @ism:derivativelyClassifiedBy
            
            Human Readable: If the Classified National Security Information
            Executive Order applies to the document, then a classification authority must be
            specified. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M319"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M319"/>
   <xsl:template match="@*|node()" priority="-2" mode="M319">
      <xsl:apply-templates select="*" mode="M319"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00143-->


	<!--RULE ISM-ID-00143-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and @ism:derivativelyClassifiedBy]"
                 priority="1000"
                 mode="M320">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and @ism:derivativelyClassifiedBy]"
                       id="ISM-ID-00143-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:derivedFrom"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:derivedFrom">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00143][Error] If ISM_USGOV_RESOURCE and attribute @ism:derivativelyClassifiedBy is specified, 
            then attribute @ism:derivedFrom must be specified. 
            
            Human Readable: Derivatively Classified data including DOE data requires
            a derived from value to be identified.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M320"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M320"/>
   <xsl:template match="@*|node()" priority="-2" mode="M320">
      <xsl:apply-templates select="*" mode="M320"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00168-->


	<!--RULE ISM-ID-00168-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('DISPLAYONLY')))]"
                 priority="1000"
                 mode="M335">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('DISPLAYONLY')))]"
                       id="ISM-ID-00168-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:displayOnlyTo)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="not(@ism:displayOnlyTo)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00168][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls is not specified or is specified and does not contain the name token 
            [DISPLAYONLY], then attribute @ism:displayOnlyTo must not be specified.
            
            Human Readable: If a portion in a USA document is not marked for DISPLAY ONLY dissemination, 
            it must not list countries to which it may be disclosed. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M335"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M335"/>
   <xsl:template match="@*|node()" priority="-2" mode="M335">
      <xsl:apply-templates select="*" mode="M335"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00176-->


	<!--RULE ISM-ID-00176-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD', 'FRD'))]"
                 priority="1000"
                 mode="M341">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD', 'FRD'))]"
                       id="ISM-ID-00176-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not($ISM_RESOURCE_ELEMENT/@ism:declassDate or $ISM_RESOURCE_ELEMENT/@ism:declassEvent)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not($ISM_RESOURCE_ELEMENT/@ism:declassDate or $ISM_RESOURCE_ELEMENT/@ism:declassEvent)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00176][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:atomicEnergyMarkings has a name token containing [RD] or [FRD], 
            then attributes @ism:declassDate and @ism:declassEvent cannot be specified
            on the resourceElement.
            
            Human Readable: Automatic declassification of documents containing 
            RD or FRD information is prohibited. Attributes declassDate and 
            declassEvent cannot be used in the classification authority block when 
            RD or FRD is present.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M341"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M341"/>
   <xsl:template match="@*|node()" priority="-2" mode="M341">
      <xsl:apply-templates select="*" mode="M341"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00213-->


	<!--RULE ISM-ID-00213-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('DISPLAYONLY'))]"
                 priority="1000"
                 mode="M370">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('DISPLAYONLY'))]"
                       id="ISM-ID-00213-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:displayOnlyTo"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:displayOnlyTo">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00213][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls contains the name token [DISPLAYONLY], then 
            attribute @ism:displayOnlyTo must be specified.
            
            Human Readable: A USA document with DISPLAY ONLY dissemination must 
            indicate the countries to which it may be disclosed.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M370"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M370"/>
   <xsl:template match="@*|node()" priority="-2" mode="M370">
      <xsl:apply-templates select="*" mode="M370"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00221-->


	<!--RULE ISM-ID-00221-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and @ism:derivativelyClassifiedBy]"
                 priority="1000"
                 mode="M374">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and @ism:derivativelyClassifiedBy]"
                       id="ISM-ID-00221-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:classificationReason or @ism:classifiedBy)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(@ism:classificationReason or @ism:classifiedBy)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
	          [ISM-ID-00221][Error] If ISM_USGOV_RESOURCE and attribute 
	          @ism:derivativelyClassifiedBy is specified, then attributes @ism:classificationReason
	          or @ism:classifiedBy must not be specified.
	          
	          Human Readable: USA documents that are derivatively classified must not
	          specify a classification reason or classified by.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M374"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M374"/>
   <xsl:template match="@*|node()" priority="-2" mode="M374">
      <xsl:apply-templates select="*" mode="M374"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00226-->


	<!--RULE ISM-ID-00226-R1-->
<xsl:template match="*[@ism:noticeType]" priority="1000" mode="M376">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:noticeType]"
                       id="ISM-ID-00226-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:unregisteredNoticeType)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(@ism:unregisteredNoticeType)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00226][Error] Attributes @ism:noticeType and @ism:unregisteredNoticeType
            may not both be used on the same element. 
            
            Human Readable: Ensure that the ISM attributes noticeType and
            unregisteredNoticeType are not used on the same element.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M376"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M376"/>
   <xsl:template match="@*|node()" priority="-2" mode="M376">
      <xsl:apply-templates select="*" mode="M376"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00250-->


	<!--RULE ISM-ID-00250-R1-->
<xsl:template match="ism:Notice[$ISM_USGOV_RESOURCE]" priority="1000" mode="M386">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ism:Notice[$ISM_USGOV_RESOURCE]"
                       id="ISM-ID-00250-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:noticeType or @ism:unregisteredNoticeType"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:noticeType or @ism:unregisteredNoticeType">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00250][Error] If ISM_USGOV_RESOURCE, element ism:Notice must specify 
		    	attribute @ism:noticeType or @ism:unregisteredNoticeType.
		    	
		    	Human Readable: Notices must specify their type.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M386"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M386"/>
   <xsl:template match="@*|node()" priority="-2" mode="M386">
      <xsl:apply-templates select="*" mode="M386"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00299-->


	<!--RULE ISM-ID-00299-R1-->
<xsl:template match="*[util:containsAnyTokenMatching(@ism:declassException, ('AEA'))]"
                 priority="1000"
                 mode="M434">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[util:containsAnyTokenMatching(@ism:declassException, ('AEA'))]"
                       id="ISM-ID-00299-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:atomicEnergyMarkings"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:atomicEnergyMarkings">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
		    	[ISM-ID-00299][Error] If an element contains the attribute @ism:declassException with a value of [AEA], 
		    	it must also contain the attribute @ism:atomicEnergyMarkings.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M434"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M434"/>
   <xsl:template match="@*|node()" priority="-2" mode="M434">
      <xsl:apply-templates select="*" mode="M434"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00324-->


	<!--RULE ISM-ID-00324-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(@ism:classification='U' and util:isUncaveatedAndNoFDR(.)) and not(@ism:compilationReason)]"
                 priority="1000"
                 mode="M446">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(@ism:classification='U' and util:isUncaveatedAndNoFDR(.)) and not(@ism:compilationReason)]"
                       id="ISM-ID-00324-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count($partTags) &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="count($partTags) &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00324][Error] If a document is ISM_USGOV_RESOURCE, it must contain portion markings. 
            
            Human Readable: All valid ISM_USGOV_RESOURCE documents must also contain portion markings.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M446"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M446"/>
   <xsl:template match="@*|node()" priority="-2" mode="M446">
      <xsl:apply-templates select="*" mode="M446"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00326-->


	<!--RULE ISM-ID-00326-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))]"
                 priority="1000"
                 mode="M448">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))]"
                       id="ISM-ID-00326-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="/*//ntk:AccessPolicy[.='urn:us:gov:ic:aces:ntk:oc']"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="/*//ntk:AccessPolicy[.='urn:us:gov:ic:aces:ntk:oc']">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
         [ISM-ID-00326][Error] ORCON information (i.e. @ism:disseminationControls of the resource node contains [OC]) 
         requires ORCON profile NTK metadata.
      </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M448"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M448"/>
   <xsl:template match="@*|node()" priority="-2" mode="M448">
      <xsl:apply-templates select="*" mode="M448"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00328-->


	<!--RULE ISM-ID-00328-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('FOUO')) and util:containsAnyOfTheTokens(@ism:classification, ('U'))]"
                 priority="1000"
                 mode="M450">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('FOUO')) and util:containsAnyOfTheTokens(@ism:classification, ('U'))]"
                       id="ISM-ID-00328-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:nonICmarkings)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="not(@ism:nonICmarkings)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00328][Error] If ISM_USGOV_RESOURCE and: 
            1. Any element in the document that has the attribute @ism:disseminationControls containing [FOUO]
            AND
            2. Has the attribute @ism:classification [U]
            
            Then the element can't have any @ism:nonICMarkings.
            
            Human Readable: Non-IC dissemination control markings in elements of USA Unclassified documents 
            supersede and take precedence over FOUO.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M450"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M450"/>
   <xsl:template match="@*|node()" priority="-2" mode="M450">
      <xsl:apply-templates select="*" mode="M450"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00349-->


	<!--RULE ISM-ID-00349-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('PR'))]"
                 priority="1000"
                 mode="M462">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('PR'))]"
                       id="ISM-ID-00349-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="/*//ntk:AccessPolicy[starts-with(.,'urn:us:gov:ic:aces:ntk:propin:')]"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="/*//ntk:AccessPolicy[starts-with(.,'urn:us:gov:ic:aces:ntk:propin:')]">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
         [ISM-ID-00349][Error] If ISM_USGOV_RESOURCE, PROPIN information (i.e. @ism:disseminationControls of the resource
         node contains [PR]) requires PROPIN NTK metadata.
      </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M462"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M462"/>
   <xsl:template match="@*|node()" priority="-2" mode="M462">
      <xsl:apply-templates select="*" mode="M462"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00350-->


	<!--RULE ISM-ID-00350-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('XD'))]"
                 priority="1000"
                 mode="M463">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('XD'))]"
                       id="ISM-ID-00350-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="/*//ntk:AccessPolicy[.='urn:us:gov:ic:aces:ntk:xd']"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="/*//ntk:AccessPolicy[.='urn:us:gov:ic:aces:ntk:xd']">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
         [ISM-ID-00350][Error] Exclusive Distribution information (i.e. @ism:nonICmarkings of the
         resource node contains [XD]) requires XD profile NTK metadata.
      </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M463"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M463"/>
   <xsl:template match="@*|node()" priority="-2" mode="M463">
      <xsl:apply-templates select="*" mode="M463"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00351-->


	<!--RULE ISM-ID-00351-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('ND'))]"
                 priority="1000"
                 mode="M464">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('ND'))]"
                       id="ISM-ID-00351-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="/*//ntk:AccessPolicy[.='urn:us:gov:ic:aces:ntk:nd']"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="/*//ntk:AccessPolicy[.='urn:us:gov:ic:aces:ntk:nd']">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
         [ISM-ID-00351][Error] No Distribution information (i.e. @ism:nonICmarkings of the resource
         node contains [ND]) requires ND profile NTK metadata.
      </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M464"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M464"/>
   <xsl:template match="@*|node()" priority="-2" mode="M464">
      <xsl:apply-templates select="*" mode="M464"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00367-->


	<!--RULE ISM-ID-00367-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and @ism:derivedFrom]"
                 priority="1000"
                 mode="M476">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and @ism:derivedFrom]"
                       id="ISM-ID-00367-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:classifiedBy)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="not(@ism:classifiedBy)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
	          [ISM-ID-00367][Error] If ISM_USGOV_RESOURCE and attribute @ism:derivedFrom is 
	          specified, then attribute @ism:classifiedBy must not be specified.
	          
	          Human Readable: USA documents that specify a derivative classifier must not also 
	          include information related to Original Classification Authorities (classificationReason and classifiedBy).
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M476"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M476"/>
   <xsl:template match="@*|node()" priority="-2" mode="M476">
      <xsl:apply-templates select="*" mode="M476"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00385-->


	<!--RULE ISM-ID-00385-R1-->
<xsl:template match="*[@ism:declassEvent]" priority="1000" mode="M487">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:declassEvent]"
                       id="ISM-ID-00385-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test=".[@ism:declassDate]"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test=".[@ism:declassDate]">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00385][Error]Attribute @ism:declassEvent requires use of attribute @ism:declassDate. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M487"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M487"/>
   <xsl:template match="@*|node()" priority="-2" mode="M487">
      <xsl:apply-templates select="*" mode="M487"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00476-->


	<!--RULE ISM-ID-00476-R1-->
<xsl:template match="*[$ISM_USCUIONLY_RESOURCE]" priority="1000" mode="M520">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USCUIONLY_RESOURCE]"
                       id="ISM-ID-00476-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:SARIdentifier or @ism:SCIcontrols or @ism:atomicEnergyMarkings or @ism:FGIsourceOpen or @ism:FGIsourceProtected)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(@ism:SARIdentifier or @ism:SCIcontrols or @ism:atomicEnergyMarkings or @ism:FGIsourceOpen or @ism:FGIsourceProtected)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [ISM-ID-00476][Error] If @ism:compliesWith="USA-CUI-ONLY",
            then attributes @ism:SCIcontrols, @ism:SARIdentifier, @ism:atomicEnergyMarkings,
            @ism:FGIsourceOpen and @ism:FGIsourceProtected must not be specified. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M520"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M520"/>
   <xsl:template match="@*|node()" priority="-2" mode="M520">
      <xsl:apply-templates select="*" mode="M520"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00486-->


	<!--RULE ISM-ID-00486-R1-->
<xsl:template match="*[($ISM_USCUIONLY_RESOURCE or $ISM_USCUI_RESOURCE) and (generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT))]"
                 priority="1000"
                 mode="M529">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USCUIONLY_RESOURCE or $ISM_USCUI_RESOURCE) and (generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT))]"
                       id="ISM-ID-00486-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(*/@ism:nonICmarkings)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(*/@ism:nonICmarkings)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00486][Error] If ISM_USCUIONLY_RESOURCE or ISM_USCUI_RESOURCE then attribute @ism:nonICmarkings must not be specified.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M529"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M529"/>
   <xsl:template match="@*|node()" priority="-2" mode="M529">
      <xsl:apply-templates select="*" mode="M529"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00494-->


	<!--RULE ISM-ID-00494-R1-->
<xsl:template match="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (util:containsAnyOfTheTokens(@ism:cuiBasic, ('PROPIN')) or util:containsAnyOfTheTokens(@ism:cuiSpecified, ('PROPIN')))]"
                 priority="1000"
                 mode="M534">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and (util:containsAnyOfTheTokens(@ism:cuiBasic, ('PROPIN')) or util:containsAnyOfTheTokens(@ism:cuiSpecified, ('PROPIN')))]"
                       id="ISM-ID-00494-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="/*//ntk:AccessPolicy[starts-with(., 'urn:us:gov:ic:aces:ntk:propin:')]"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="/*//ntk:AccessPolicy[starts-with(., 'urn:us:gov:ic:aces:ntk:propin:')]">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
         [ISM-ID-00494][Error] If ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, then if
         the document contains a PROPIN CUI Category marking (either Basic or Specified), then the
         document MUST have PROPIN_NTK metadata.
         
         Human Readable: PROPIN CUI information (either @ism:cuiBasic or
         @ism:cuiSpecified contains 'PROPIN') requires PROPIN NTK metadata.
      </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M534"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M534"/>
   <xsl:template match="@*|node()" priority="-2" mode="M534">
      <xsl:apply-templates select="*" mode="M534"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00495-->


	<!--RULE ISM-ID-00495-R1-->
<xsl:template match="*[@ism:* and $ISM_USCUIONLY_RESOURCE]"
                 priority="1000"
                 mode="M535">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:* and $ISM_USCUIONLY_RESOURCE]"
                       id="ISM-ID-00495-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:classification or @ism:ownerProducer)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(@ism:classification or @ism:ownerProducer)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00495][Error] If @ism:compliesWith="USA-CUI-ONLY" then attributes
            @ism:classification and @ism:ownerProducer must not be specified. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M535"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M535"/>
   <xsl:template match="@*|node()" priority="-2" mode="M535">
      <xsl:apply-templates select="*" mode="M535"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00497-->


	<!--RULE ISM-ID-00497-R1-->
<xsl:template match="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and (generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)) and (@ism:cuiBasic or @ism:cuiSpecified)]"
                 priority="1000"
                 mode="M537">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and (generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)) and (@ism:cuiBasic or @ism:cuiSpecified)]"
                       id="ISM-ID-00497-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:cuiControlledBy"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:cuiControlledBy">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00497][Error] If a document contains either @ism:cuiBasic or @ism:cuiSpecified, 
            then the document must contain @ism:cuiControlledBy.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M537"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M537"/>
   <xsl:template match="@*|node()" priority="-2" mode="M537">
      <xsl:apply-templates select="*" mode="M537"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00499-->


	<!--RULE ISM-ID-00499-R1-->
<xsl:template match="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and (generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT))]"
                 priority="1000"
                 mode="M539">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and (generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT))]"
                       id="ISM-ID-00499-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:cuiControlledBy"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:cuiControlledBy">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00499][Error] If a document has @ism:complieswith="USA-CUI" or "USA-CUI-ONLY", 
            then it must contain @ism:cuiControlledBy.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M539"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M539"/>
   <xsl:template match="@*|node()" priority="-2" mode="M539">
      <xsl:apply-templates select="*" mode="M539"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00512-->


	<!--RULE ISM-ID-00512-R1-->
<xsl:template match="*[($ISM_USGOV_RESOURCE or $ISM_USCUIONLY_RESOURCE) and util:containsAnyOfTheTokens(@ism:secondBannerLine, 'HVCO')]"
                 priority="1000"
                 mode="M548">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USGOV_RESOURCE or $ISM_USCUIONLY_RESOURCE) and util:containsAnyOfTheTokens(@ism:secondBannerLine, 'HVCO')]"
                       id="ISM-ID-00512-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:handleViaChannels"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:handleViaChannels">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00512][Error] If ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, and attribute 
            @ism:secondBannerLine contains the name token [HVCO], then attribute @ism:handleViaChannels must be specified.
            
            Human Readable: USA documents containing Handle Via Channels Only in the second banner line
            must specify to which channels the document is restricted.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M548"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M548"/>
   <xsl:template match="@*|node()" priority="-2" mode="M548">
      <xsl:apply-templates select="*" mode="M548"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00513-->


	<!--RULE ISM-ID-00513-R1-->
<xsl:template match="*[($ISM_USGOV_RESOURCE or $ISM_USCUIONLY_RESOURCE) and not(util:containsAnyOfTheTokens(@ism:secondBannerLine, 'HVCO'))]"
                 priority="1000"
                 mode="M549">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USGOV_RESOURCE or $ISM_USCUIONLY_RESOURCE) and not(util:containsAnyOfTheTokens(@ism:secondBannerLine, 'HVCO'))]"
                       id="ISM-ID-00513-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(@ism:handleViaChannels)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(@ism:handleViaChannels)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00513][Error] If ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, and attribute 
            @ism:handleViaChannels is specified, then @ism:secondBannerLine MUST contain the name token [HVCO].
            
            Human Readable: USA documents that specify Handle Via Channels MUST specify [HVCO] in the @ism:secondBannerLine attribute.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M549"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M549"/>
   <xsl:template match="@*|node()" priority="-2" mode="M549">
      <xsl:apply-templates select="*" mode="M549"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00518-->


	<!--RULE ISM-ID-00518-R1-->
<xsl:template match="ism:Notice[$ISM_USGOV_RESOURCE and exists(@ism:noticeProseID)] | ism:NoticeExternal[$ISM_USGOV_RESOURCE and exists(@ism:noticeProseID)]"
                 priority="1000"
                 mode="M554">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ism:Notice[$ISM_USGOV_RESOURCE and exists(@ism:noticeProseID)] | ism:NoticeExternal[$ISM_USGOV_RESOURCE and exists(@ism:noticeProseID)]"
                       id="ISM-ID-00518-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(exists(.//ism:NoticeText))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(exists(.//ism:NoticeText))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00518][Error] For ism:Notice or ism:NoticeExternal, if @ism:noticeProseID is present then ism:NoticeText is prohibited.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M554"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M554"/>
   <xsl:template match="@*|node()" priority="-2" mode="M554">
      <xsl:apply-templates select="*" mode="M554"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00519-->


	<!--RULE ISM-ID-00519-R1-->
<xsl:template match="ism:Notice[$ISM_USGOV_RESOURCE and not(exists(@ism:noticeProseID))] | ism:NoticeExternal[$ISM_USGOV_RESOURCE and not(exists(@ism:noticeProseID))]"
                 priority="1000"
                 mode="M555">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="ism:Notice[$ISM_USGOV_RESOURCE and not(exists(@ism:noticeProseID))] | ism:NoticeExternal[$ISM_USGOV_RESOURCE and not(exists(@ism:noticeProseID))]"
                       id="ISM-ID-00519-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="exists(.//ism:NoticeText)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="exists(.//ism:NoticeText)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00519][Error] For ism:Notice or ism:NoticeExternal, if @ism:noticeProseID is absent then ism:NoticeText is required.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M555"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M555"/>
   <xsl:template match="@*|node()" priority="-2" mode="M555">
      <xsl:apply-templates select="*" mode="M555"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00522-->


	<!--RULE ISM-ID-00522-R1-->
<xsl:template match="*[$ISM_NSI_EO_APPLIES and @ism:highWaterNATO]"
                 priority="1000"
                 mode="M557">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_NSI_EO_APPLIES and @ism:highWaterNATO]"
                       id="ISM-ID-00522-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="                 (matches(normalize-space(string(@ism:ownerProducer)), 'NATO') or                 matches(normalize-space(string(@ism:FGIsourceOpen)), 'NATO') or                 matches(normalize-space(string(@ism:FGIsourceProtected)), 'FGI'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="(matches(normalize-space(string(@ism:ownerProducer)), 'NATO') or matches(normalize-space(string(@ism:FGIsourceOpen)), 'NATO') or matches(normalize-space(string(@ism:FGIsourceProtected)), 'FGI'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [ISM-ID-00522][Error] If ISM_NSI_EO_APPLIES and the
            @ism:highWaterNATO attribute exists on a banner or portion, then @ism:FGIsourceOpen
            contains [NATO] or @ism:ownerProducer contains [NATO] or @ism:FGIsourceProtected
            contains [FGI]. Human Readable: For documents under E.O. 13526, the NATO high-water
            indicator can only exist on an element where either @ism:FGIsourceOpen contains [NATO]
            or @ism:ownerProducer contains [NATO] or @ism:FGIsourceProtected contains [FGI].
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M557"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M557"/>
   <xsl:template match="@*|node()" priority="-2" mode="M557">
      <xsl:apply-templates select="*" mode="M557"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00523-->


	<!--RULE ISM-ID-00523-R1-->
<xsl:template match="*[$ISM_NSI_EO_APPLIES and contains(@ism:FGIsourceOpen,'NATO')]"
                 priority="1000"
                 mode="M558">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_NSI_EO_APPLIES and contains(@ism:FGIsourceOpen,'NATO')]"
                       id="ISM-ID-00523-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:highWaterNATO"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:highWaterNATO">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [ISM-ID-00523][Error] If ISM_NSI_EO_APPLIES and @ism:FGIsourceOpen
                contains [NATO] on a banner or portion, then @ism:highWaterNATO must be present.
                Human Readable: For documents under E.O. 13526, the NATO high-water indicator must exist on an
                element when @ism:FGIsourceOpen contains [NATO] </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M558"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M558"/>
   <xsl:template match="@*|node()" priority="-2" mode="M558">
      <xsl:apply-templates select="*" mode="M558"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00524-->


	<!--RULE ISM-ID-00524-R1-->
<xsl:template match="*[$ISM_NSI_EO_APPLIES and @ism:highWaterNATO]"
                 priority="1000"
                 mode="M559">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_NSI_EO_APPLIES and @ism:highWaterNATO]"
                       id="ISM-ID-00524-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(./@ism:ownerProducer='NATO')"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(./@ism:ownerProducer='NATO')">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [ISM-ID-00524][Error] If ISM_NSI_EO_APPLIES and the
            @ism:highWaterNATO attribute exists on a banner or portion, then @ism:ownerProducer
            cannot be equal to 'NATO'. Human Readable: For documents under E.O. 13526, the NATO
            high-water indicator is not allowed where @ism:ownerProducer='NATO'. It is okay if
            @ism:ownerProducer contains 'NATO' and other tokens like 'USA'. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M559"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M559"/>
   <xsl:template match="@*|node()" priority="-2" mode="M559">
      <xsl:apply-templates select="*" mode="M559"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00525-->


	<!--RULE ISM-ID-00525-R1-->
<xsl:template match="*[$ISM_NSI_EO_APPLIES and @ism:highWaterNATO]"
                 priority="1000"
                 mode="M560">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_NSI_EO_APPLIES and @ism:highWaterNATO]"
                       id="ISM-ID-00525-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if (normalize-space(string(./@ism:highWaterNATO))='NATO-TS' and normalize-space(string(./@ism:classification))='TS')                 then true()             else if (normalize-space(string(./@ism:highWaterNATO))='NATO-S' and (normalize-space(string(./@ism:classification))='S'             or normalize-space(string(./@ism:classification))='TS'))                 then true()             else if (normalize-space(string(./@ism:highWaterNATO))='NATO-C' and (normalize-space(string(./@ism:classification))='TS'              or normalize-space(string(./@ism:classification))='S' or normalize-space(string(./@ism:classification))='C'))                 then true()             else if (normalize-space(string(./@ism:highWaterNATO))='NATO-R' and not(normalize-space(string(./@ism:classification))='U'))                 then true()             else if (normalize-space(string(./@ism:highWaterNATO))='NATO-U') then true()             else false()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if (normalize-space(string(./@ism:highWaterNATO))='NATO-TS' and normalize-space(string(./@ism:classification))='TS') then true() else if (normalize-space(string(./@ism:highWaterNATO))='NATO-S' and (normalize-space(string(./@ism:classification))='S' or normalize-space(string(./@ism:classification))='TS')) then true() else if (normalize-space(string(./@ism:highWaterNATO))='NATO-C' and (normalize-space(string(./@ism:classification))='TS' or normalize-space(string(./@ism:classification))='S' or normalize-space(string(./@ism:classification))='C')) then true() else if (normalize-space(string(./@ism:highWaterNATO))='NATO-R' and not(normalize-space(string(./@ism:classification))='U')) then true() else if (normalize-space(string(./@ism:highWaterNATO))='NATO-U') then true() else false()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [ISM-ID-00525][Error] If ISM_NSI_EO_APPLIES and the @ism:highWaterNATO
            attribute exists on a banner or portion, then @ism:highWaterNATO cannot be higher than @ism:classification.
            Human Readable: For documents under E.O. 13526, the NATO high-water indicator value cannot be
            higher than the classification value. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M560"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M560"/>
   <xsl:template match="@*|node()" priority="-2" mode="M560">
      <xsl:apply-templates select="*" mode="M560"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00526-->


	<!--RULE ISM-ID-00526-R1-->
<xsl:template match="*[$ISM_NSI_EO_APPLIES and contains(@ism:ownerProducer,'NATO') and not(@ism:ownerProducer='NATO')]"
                 priority="1000"
                 mode="M561">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_NSI_EO_APPLIES and contains(@ism:ownerProducer,'NATO') and not(@ism:ownerProducer='NATO')]"
                       id="ISM-ID-00526-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:highWaterNATO"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:highWaterNATO">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> [ISM-ID-00526][Error] If ISM_NSI_EO_APPLIES and @ism:ownerProducer
            attribute contains multiple values on a banner or portion, one being NATO, then @ism:highWaterNATO must exist.
            Human Readable: For documents under E.O. 13526, the NATO high-water indicator must exist on an
            element when @ism:ownerProducer attribute contains multiple values and NATO. </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M561"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M561"/>
   <xsl:template match="@*|node()" priority="-2" mode="M561">
      <xsl:apply-templates select="*" mode="M561"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00529-->


	<!--RULE ISM-ID-00529-R1-->
<xsl:template match="*[@ism:SARIdentifier]" priority="1000" mode="M563">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:SARIdentifier]"
                       id="ISM-ID-00529-R1"/>
      <xsl:variable name="nonmatchingTokens"
                    select="for $token in tokenize(normalize-space(string(@ism:SARIdentifier)), ' ')     return if (not(matches($token,'^SAR-[A-Z]{3,}:((C|S|TS):){0,1}[A-Za-z0-9._-]{1,}$'))) then $token else null"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count($nonmatchingTokens) = 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count($nonmatchingTokens) = 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[ISM-ID-00529][Error] All tokens in the @ism:SARIdentifier attribute MUST conform to the regex 
			^SAR-[A-Z]{3,}:((C|S|TS):){0,1}[A-Za-z0-9._-]{1,}$ . Human Readable:  All tokens in @ism:SARIdentifier must conform to
			a regular expression for: SAR-SourceAuthority:Classification:SAPmarking or SAR-SourceAuthority:SAPmarking.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M563"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M563"/>
   <xsl:template match="@*|node()" priority="-2" mode="M563">
      <xsl:apply-templates select="*" mode="M563"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00531-->


	<!--RULE ISM-ID-00531-R1-->
<xsl:template match="*[@ism:resourceElement='true' and @ism:SARIdentifier and $ISM_USDOD_RESOURCE and $ISM_USIC_RESOURCE]"
                 priority="1000"
                 mode="M565">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:resourceElement='true' and @ism:SARIdentifier and $ISM_USDOD_RESOURCE and $ISM_USIC_RESOURCE]"
                       id="ISM-ID-00531-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:countSARmarkings(./@ism:SARIdentifier) = 1"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:countSARmarkings(./@ism:SARIdentifier) = 1">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[ISM-ID-00531][Error] All resource elements with SAR markings that contain @ism:compliesWith="USGov USDOD USIC attribute MUST contain 
			only one token in @ism:SARIdentifier. This allows @ism:SARIdentifier to have multiple tokens, but disallows having multiple tokens 
			and @ism:compliesWith containing both USDOD and USIC. This rule satisfies requirements specified in the IC and DoD authoritative sources  
			for SAP policies; DoD Directive 5205.07 - Special Access Program (SAP) Policy and (2) IC Markings System Register and Manual.
		</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M565"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M565"/>
   <xsl:template match="@*|node()" priority="-2" mode="M565">
      <xsl:apply-templates select="*" mode="M565"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00532-->


	<!--RULE ISM-ID-00532-R1-->
<xsl:template match="*[@ism:SARIdentifier]" priority="1000" mode="M566">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:SARIdentifier]"
                       id="ISM-ID-00532-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="if (contains(string(./@ism:SARIdentifier),':TS:')) then                   (if (normalize-space(string(./@ism:classification))='TS') then true() else false() )              else if (contains(string(./@ism:SARIdentifier),':S:')) then                  (if ((normalize-space(string(./@ism:classification))='S' or normalize-space(string(./@ism:classification))='TS')) then true()                     else false() )             else if (contains(string(./@ism:SARIdentifier),':C:')) then                  (if ((normalize-space(string(./@ism:classification))='TS' or normalize-space(string(./@ism:classification))='S'                      or normalize-space(string(./@ism:classification))='C')) then true() else false() )             else if (not(contains(./@ism:SARIdentifier,':TS:') or contains(string(./@ism:SARIdentifier),':S:') or                     contains(./@ism:SARIdentifier,':C:'))) then true()                             else false()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="if (contains(string(./@ism:SARIdentifier),':TS:')) then (if (normalize-space(string(./@ism:classification))='TS') then true() else false() ) else if (contains(string(./@ism:SARIdentifier),':S:')) then (if ((normalize-space(string(./@ism:classification))='S' or normalize-space(string(./@ism:classification))='TS')) then true() else false() ) else if (contains(string(./@ism:SARIdentifier),':C:')) then (if ((normalize-space(string(./@ism:classification))='TS' or normalize-space(string(./@ism:classification))='S' or normalize-space(string(./@ism:classification))='C')) then true() else false() ) else if (not(contains(./@ism:SARIdentifier,':TS:') or contains(string(./@ism:SARIdentifier),':S:') or contains(./@ism:SARIdentifier,':C:'))) then true() else false()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>[ISM-ID-00532][Error] For all elements with ism:SARIdentifier with tokens
            that include classification portion marks (e.g., DOD:TS:aaaa or DOD:C:bbbb), the value of
            the classification portion mark cannot be higher than @ism:classification on the same
            element. Human Readable: For @ism:SARIdentifier tokens that include classification portion
            marks in their values, the classification portion mark cannot be higher than the
            classification value. Note that some @ism:SARIdentifier tokens may not contain
            classification portion marks, e.g., DNI:kkkk; the rule does not apply to these tokens. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M566"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M566"/>
   <xsl:template match="@*|node()" priority="-2" mode="M566">
      <xsl:apply-templates select="*" mode="M566"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00533-->


	<!--RULE ISM-ID-00533-R1-->
<xsl:template match="*[@ism:resourceElement='true' and @ism:SARIdentifier and $ISM_USDOD_RESOURCE and $ISM_USIC_RESOURCE]"
                 priority="1000"
                 mode="M567">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:resourceElement='true' and @ism:SARIdentifier and $ISM_USDOD_RESOURCE and $ISM_USIC_RESOURCE]"
                       id="ISM-ID-00533-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:countSARmarkings(./@ism:SARIdentifier) &lt; 3"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:countSARmarkings(./@ism:SARIdentifier) &lt; 3">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00533][Error] All resource elements that contain @ism:compliesWith="USGov USDOD USIC" attribute MUST contain no more than two (2) tokens
            in @ism:SARIdentifier. This rule satisfies requirements specified in the IC and DoD authoritative sources for SAP policies; [1] DoD Directive 
            5205.07 - Special Access Program (SAP) Policy and [2] IC Markings System Register and Manual.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M567"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M567"/>
   <xsl:template match="@*|node()" priority="-2" mode="M567">
      <xsl:apply-templates select="*" mode="M567"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00534-->


	<!--RULE ISM-ID-00534-R1-->
<xsl:template match="*[@ism:SARIdentifier and $ISM_USDOD_RESOURCE and $ISM_USIC_RESOURCE]"
                 priority="1000"
                 mode="M568">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ism:SARIdentifier and $ISM_USDOD_RESOURCE and $ISM_USIC_RESOURCE]"
                       id="ISM-ID-00534-R1"/>
      <xsl:variable name="SARsWithDashes"
                    select="for $token in tokenize(normalize-space(string(@ism:SARIdentifier)), ' ') return if (contains(substring-after($token,'SAR-'),'-')) then $token              else null"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count($SARsWithDashes) = 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count($SARsWithDashes) = 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00534][Error] If there are any elements with a dash (-) in @ism:SARIdentifier (excluding the SAR- prefix), then it is an ERROR if
            @ism:compliesWith="USGov USDOD USIC".  This is an ERROR because IC rules state that a dash in @ism:SARIdentifier
            indicates a compartment or subcompartment.  A DoD @ism:SARIdentifier with a dash is just a plain SAP marking 
            containing a dash; DoD SAPs do not have compartments or subcompartments. This means DoD and IC rules differ on 
            how to render SAP markings containing dashes; therefore, it is not allowed to have SAPs with dashes in a document 
            that complies with both DoD and IC rules.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M568"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M568"/>
   <xsl:template match="@*|node()" priority="-2" mode="M568">
      <xsl:apply-templates select="*" mode="M568"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00535-->


	<!--RULE ISM-ID-00535-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('WAIVED'))]"
                 priority="1000"
                 mode="M569">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('WAIVED'))]"
                       id="ISM-ID-00535-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="$ISM_USDOD_RESOURCE"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="$ISM_USDOD_RESOURCE">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00535][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls contains the name token [WAIVED], then 
            attribute @ism:compliesWith must contain [USDOD]. 
            Human Readable: USA documents containing the WAIVED 
            dissemination control must comply with USDOD rules.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M569"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M569"/>
   <xsl:template match="@*|node()" priority="-2" mode="M569">
      <xsl:apply-templates select="*" mode="M569"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00012-->


	<!--RULE ISM-ID-00012-R1-->
<xsl:template match="*[((@ism:* except (@ism:pocType | @ism:DESVersion | @ism:unregisteredNoticeType | @ism:ISMCATCESVersion))          or (self::arh:ExternalSecurity or self::ntk:Access or self::ntk:ExternalAccess or self::ntk:AccessProfile))         and not($ISM_USCUIONLY_RESOURCE)]"
                 priority="1000"
                 mode="M574">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[((@ism:* except (@ism:pocType | @ism:DESVersion | @ism:unregisteredNoticeType | @ism:ISMCATCESVersion))          or (self::arh:ExternalSecurity or self::ntk:Access or self::ntk:ExternalAccess or self::ntk:AccessProfile))         and not($ISM_USCUIONLY_RESOURCE)]"
                       id="ISM-ID-00012-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:ownerProducer and @ism:classification"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="@ism:ownerProducer and @ism:classification">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00012][Error] If the document does NOT have @ism:compliesWith="USA-CUI-ONLY",
            then if:
            1. any of the attributes defined in this DES other than @ism:DESVersion, @ism:ISMCATCESVersion,
            @ism:unregisteredNoticeType, or @ism:pocType are specified for an element, 
            OR 
            2. the current node is one of elements arh:Security, arh:ExternalSecurity, ntk:Access, or ntk:AccessProfile,
            then attributes @ism:classification and @ism:ownerProducer must be specified for the element.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M574"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M574"/>
   <xsl:template match="@*|node()" priority="-2" mode="M574">
      <xsl:apply-templates select="*" mode="M574"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00102-->


	<!--RULE ISM-ID-00102-R1-->
<xsl:template match="/*[descendant-or-self::*[@ism:* except (@ism:ISMCATCESVersion)]]"
                 priority="1000"
                 mode="M575">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="/*[descendant-or-self::*[@ism:* except (@ism:ISMCATCESVersion)]]"
                       id="ISM-ID-00102-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $element in descendant-or-self::node() satisfies $element/@ism:DESVersion"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $element in descendant-or-self::node() satisfies $element/@ism:DESVersion">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00102][Error] The attribute @ism:DESVersion in the namespace urn:us:gov:ic:ism must be specified.
            
            Human Readable: The data encoding specification version must be specified.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M575"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M575"/>
   <xsl:template match="@*|node()" priority="-2" mode="M575">
      <xsl:apply-templates select="*" mode="M575"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00118-->


	<!--RULE ISM-ID-00118-R1-->
<xsl:template match="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)][1]"
                 priority="1000"
                 mode="M577">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)][1]"
                       id="ISM-ID-00118-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:createDate"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:createDate">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00118][Error] The first element in document order having @ism:resourceElement specified with a value of [true] 
            must have @ism:createDate specified.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M577"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M577"/>
   <xsl:template match="@*|node()" priority="-2" mode="M577">
      <xsl:apply-templates select="*" mode="M577"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00337-->


	<!--RULE ISM-ID-00337-R1-->
<xsl:template match="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)][1]"
                 priority="1000"
                 mode="M586">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)][1]"
                       id="ISM-ID-00337-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:compliesWith"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:compliesWith">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00337][Error] The first element in document order having @ism:resourceElement specified with a value of [true] 
            must have @ism:compliesWith specified.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M586"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M586"/>
   <xsl:template match="@*|node()" priority="-2" mode="M586">
      <xsl:apply-templates select="*" mode="M586"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00449-->


	<!--RULE ISM-ID-00449-R1-->
<xsl:template match="/arh:*" priority="1000" mode="M604">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="/arh:*"
                       id="ISM-ID-00449-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="false()"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="false()">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00449][Error] The ARH elements cannot be used as root elements.
            
            Human Readable: ARH is not designed to stand-alone and therefore should never
            be used as a root element.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M604"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M604"/>
   <xsl:template match="@*|node()" priority="-2" mode="M604">
      <xsl:apply-templates select="*" mode="M604"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00450-->


	<!--RULE ISM-ID-00450-R1-->
<xsl:template match="*[@arh:DESVersion]" priority="1000" mode="M605">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@arh:DESVersion]"
                       id="ISM-ID-00450-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="not(@arh:DESVersion)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="not(@arh:DESVersion)">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00450][Warning] @arh:DESVersion is a DEPRECATED attribute. Found arh:DESVersion=<xsl:text/>
                  <xsl:value-of select="./@arh:DESVersion"/>
                  <xsl:text/>
            
            Human Readable: ARH DESVersion is a DEPRECATED attribute.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M605"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M605"/>
   <xsl:template match="@*|node()" priority="-2" mode="M605">
      <xsl:apply-templates select="*" mode="M605"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00452-->


	<!--RULE ISM-ID-00452-R1-->
<xsl:template match="*[@ntk:DESVersion]" priority="1000" mode="M607">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[@ntk:DESVersion]"
                       id="ISM-ID-00452-R1"/>

		    <!--ASSERT warning-->
<xsl:choose>
         <xsl:when test="not(@ntk:DESVersion)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="not(@ntk:DESVersion)">
               <xsl:attribute name="flag">warning</xsl:attribute>
               <xsl:attribute name="role">warning</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00452][Warning] @ntk:DESVersion is a DEPRECATED attribute.
            
            Human Readable: NTK DESVersion is a DEPRECATED attribute.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M607"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M607"/>
   <xsl:template match="@*|node()" priority="-2" mode="M607">
      <xsl:apply-templates select="*" mode="M607"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00510-->


	<!--RULE ISM-ID-00510-R1-->
<xsl:template match="arh:Security" priority="1000" mode="M609">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="arh:Security"
                       id="ISM-ID-00510-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="@ism:resourceElement"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="@ism:resourceElement">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00510][Error] arh:Security element must contain @ism:resourceElement attribute.
            
            Human Readable: arh:Security element must contain @ism:resourceElement attribute.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M609"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M609"/>
   <xsl:template match="@*|node()" priority="-2" mode="M609">
      <xsl:apply-templates select="*" mode="M609"/>
   </xsl:template>
</xsl:stylesheet>
<!--UNCLASSIFIED-->
