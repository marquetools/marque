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
         <xsl:attribute name="phase">ROLLDOWN</xsl:attribute>
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
            <xsl:attribute name="id">ISM-ID-00056</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00056</xsl:attribute>
            <svrl:text>
        [ISM-ID-00056][Error] If the document is an ISM_USGOV_RESOURCE and
        attribute @ism:classification of ISM_RESOURCE_ELEMENT has a value of [U] then no element meeting
        ISM_CONTRIBUTES in the document may have a @ism:classification attribute of [C], [S], [TS], or [R]. 
        
        Human Readable: USA UNCLASSIFIED documents cannot have portion markings with the
        classification TOP SECRET, SECRET, CONFIDENTIAL, or RESTRICTED data. 
    </svrl:text>
            <svrl:text>
        If the document is an ISM_USGOV_RESOURCE and attribute @ism:classification on $ISM_RESOURCE_ELEMENT 
        has a value of [U], this rule ensures that no element meeting ISM_CONTRIBUTES has attribute @ism:classification
        with value [C], [S], [TS],
        [R]. </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M273"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00058</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00058</xsl:attribute>
            <svrl:text>
        [ISM-ID-00058][Error] If ISM_USGOV_RESOURCE and attribute @ism:classification of ISM_RESOURCE_ELEMENT 
        has a value of [C] then no element meeting ISM_CONTRIBUTES_USA in the document may have a @ism:classification 
        attribute of [S] or [TS].
        
        Human Readable: USA CONFIDENTIAL documents cannot have TOP SECRET or SECRET data.
    </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE and attribute @ism:classification on $ISM_RESOURCE_ELEMENT has a value of [C], 
      this rule ensures that no element meeting ISM_CONTRIBUTES_USA has attribute @ism:classification with value [S], [TS]. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M274"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00059</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00059</xsl:attribute>
            <svrl:text>
        [ISM-ID-00059][Error] If ISM_USGOV_RESOURCE and attribute @ism:classification of ISM_RESOURCE_ELEMENT 
        has a value of [S] then no element meeting ISM_CONTRIBUTES_USA in the document may have 
        a @ism:classification attribute of [TS].
        
        Human Readable: USA SECRET documents can't have TOP SECRET data.
    </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE and attribute @ism:classification
      on $ISM_RESOURCE_ELEMENT has a value of [S], this rule ensures that
      no element meeting ISM_CONTRIBUTES_USA has attribute @ism:classification with
      value [TS].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M275"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00108</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00108</xsl:attribute>
            <svrl:text>
    If ISM_USGOV_RESOURCE and attribute classification of ISM_RESOURCE_ELEMENT 
    has a value of [TS] and attribute @ism:compilationReason does not have a 
    value, then this rule ensures that at least one element meeting ISM_CONTRIBUTES 
    specifies attribute classification with a value of [TS].
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M303"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00109</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00109</xsl:attribute>
            <svrl:text>
    If ISM_USGOV_RESOURCE and attribute classification of ISM_RESOURCE_ELEMENT 
    has a value of [S] and attribute @ism:compilationReason does not have a 
    value, then this rule ensures that at least one element meeting ISM_CONTRIBUTES 
    specifies attribute classification with a value of [S].
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M304"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00110</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00110</xsl:attribute>
            <svrl:text>
    If ISM_USGOV_RESOURCE and attribute classification of ISM_RESOURCE_ELEMENT 
    has a value of [C] and attribute @ism:compilationReason does not have a 
    value, then this rule ensures that at least one element meeting ISM_CONTRIBUTES 
    specifies attribute classification with a value of [C].
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M305"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00128</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00128</xsl:attribute>
            <svrl:text>
		For all elements that contribute to rollup when all of the following are true:
		(a) the given expression $ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings contains the given value 'FRD'
		(b) the given exception expression $ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings does not contain the given exception value 'RD'
		(c) $ISM_USGOV_RESOURCE is true
		
		Assert that some non-resource node element satisfies both
		(a) @ism:noticeType contains the 'FRD' token
		(b) not(@ism:externalNotice is true)

		This rule depends on $partTags defined in the ISM_XML.sch master Schematron file.
		
		The calling rule must pass $ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings, 'FRD', $ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings, 'RD'.
	</svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M308"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00132</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00132</xsl:attribute>
            <svrl:text>
        [ISM-ID-00132][Error] If ISM_USGOV_RESOURCE and the
        ISM_RESOURCE_ELEMENT has the attribute @ism:disseminationControls containing [RELIDO] then every
        element meeting ISM_CONTRIBUTES_CLASSIFIED in the document must have the attribute
        @ism:disseminationControls containing [RELIDO]. 
        
        Human Readable: USA documents having RELIDO at the resource level must have every classified portion 
        having RELIDO and on any U portions that have explicit Release specified must have RELIDO. 
    </svrl:text>
            <svrl:text> 
        If the document is an ISM_USGOV_RESOURCE, the current element is the
        ISM_RESOURCE_ELEMENT, and the ISM_RESOURCE_ELEMENT specifies the attribute
        @ism:disseminationControls with a value containing the token [RELIDO] and not an 
        unclass NF-based token (SBU-NF or LES-NF), then this rule ensures that every element 
        meeting ISM_CONTRIBUTES_CLASSIFIED specifies attribute @ism:disseminationControls 
        with a value containing the token [RELIDO]. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M311"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00154</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00154</xsl:attribute>
            <svrl:text>
    If ISM_USGOV_RESOURCE and attribute disseminationControls of ISM_RESOURCE_ELEMENT 
    has a value of [FOUO] and attribute @ism:compilationReason does not have a 
    value, then this rule ensures that at least one element meeting ISM_CONTRIBUTES 
    specifies attribute disseminationControls with a value of [FOUO].
  </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M330"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00219</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00219</xsl:attribute>
            <svrl:text>
        [ISM-ID-00219][Error] If element meets ISM_CONTRIBUTES and attribute
        @ism:ownerProducer contains the token [FGI], then attribute 
        @ism:FGIsourceProtected must have a value containing the token [FGI].
        
        Human Readable: Any non-resource element that contributes to the 
        document's banner roll-up and has FOREIGN GOVERNMENT INFORMATION (FGI)
        must also specify attribute FGIsourceProtected with token FGI.
    </svrl:text>
            <svrl:text>
        For each element which is not the $ISM_RESOURCE_ELEMENT and meets 
        ISM_CONTRIBUTES and specifies attribute @ism:ownerProducer with a value
        containing the token [FGI], this rule ensures that attribute 
        @ism:FGIsourceProtected is specified with a value containing the
        token [FGI].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M373"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00228</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00228</xsl:attribute>
            <svrl:text>
        [ISM-ID-00228][Error] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings of ISM_RESOURCE_ELEMENT contains 
        [FRD] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
        @ism:atomicEnergyMarking attribute containing [FRD].
        
        Human Readable: USA documents marked FRD at the resource level must have FRD data.
    </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE, the current element is the
      ISM_RESOURCE_ELEMENT, and attribute @ism:atomicEnergyMarkings is specified
      with a value containing the value [FRD], then this rule ensures that some
      element meeting ISM_CONTRIBUTES specifies attribute @ism:atomicEnergyMarkings
      with a value containing [FRD].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M377"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00229</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00229</xsl:attribute>
            <svrl:text>
        [ISM-ID-00229][Error] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings of ISM_RESOURCE_ELEMENT contains 
        [RD] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
        @ism:atomicEnergyMarking attribute containing [RD].
        
        Human Readable: USA documents marked RD at the resource level must have RD data.
    </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE, the current element is the
      ISM_RESOURCE_ELEMENT, and attribute @ism:atomicEnergyMarkings is specified
      with a value containing the value [RD], then this rule ensures that some
      element meeting ISM_CONTRIBUTES specifies attribute @ism:atomicEnergyMarkings
      with a value containing [RD].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M378"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00230</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00230</xsl:attribute>
            <svrl:text>
        [ISM-ID-00230][Error] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings of ISM_RESOURCE_ELEMENT contains 
        [FRD-SG-##] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
        @ism:atomicEnergyMarking attribute containing the same [FRD-SG-##].
        
        Human Readable: USA documents marked FRD-SG-## at the resource level must have FRD-SG-## data, where ## is the same.
    </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE, the current element is the
      ISM_RESOURCE_ELEMENT, and attribute @ism:atomicEnergyMarkings is specified
      with a value containing a token matching [FRD-SG-##], then this rule ensures that some
      element meeting ISM_CONTRIBUTES specifies attribute @ism:atomicEnergyMarkings
      with a value containing a token matching the same [FRD-SG-##].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M379"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00231</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00231</xsl:attribute>
            <svrl:text>
        [ISM-ID-00231][Error] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings of ISM_RESOURCE_ELEMENT contains 
        [RD-SG-##] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
        @ism:atomicEnergyMarking attribute containing the same [RD-SG-##].
        
        Human Readable: USA documents marked RD-SG-## at the resource level must have RD-SG-## or FRD-SG-## data, where ## is the same.
    </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE, the current element is the
      ISM_RESOURCE_ELEMENT, and attribute @ism:atomicEnergyMarkings is specified
      with a value containing a token matching [RD-SG-##], then this rule ensures that some
      element meeting ISM_CONTRIBUTES specifies attribute @ism:atomicEnergyMarkings
      with a value containing a token matching the same [RD-SG-##] or [FRD-SG-##].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M380"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00252</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00252</xsl:attribute>
            <svrl:text>
        [ISM-ID-00252][Error] If ISM_RESOURCE_ELEMENT specifies the attribute
        @ism:disseminationControls with a value containing the token [RELIDO], 
        then attribute @ism:nonICmarkings must not be specified with a value containing 
        the token [NNPI]. 
        
        Human Readable: NNPI tokens are not valid for documents that have
        RELIDO at the resource level.
    </svrl:text>
            <svrl:text>
        For resource elements which have attribute @ism:disseminationControls specified 
        with a value containing the token [RELIDO], this rule ensures that attribute 
        @ism:nonICmarkings is not specified with a value containing the token [NNPI].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M387"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00303</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00303</xsl:attribute>
            <svrl:text>
        [ISM-ID-00303][Error] If ISM_USGOV_RESOURCE and the document contains attribute 
        @ism:disseminationControls with name token [OC-USGOV] in the banner, then 
        all [OC] portions must also contain [OC-USGOV].
        
        Human Readable: A USA document with OC-USGOV dissemination in the banner
        must also contain OC-USGOV in any OC portions.
    </svrl:text>
            <svrl:text>
    	If the document is an ISM_USGOV_RESOURCE and the resource element
    	contains attribute @ism:disseminationControls with name token [OC-USGOV], then this rule 
    	ensures that every portion contain name token [OC] also contains name token [OC-USGOV].    	
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M436"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00316</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00316</xsl:attribute>
            <svrl:text>
        [ISM-ID-00316][Error] If ISM_USGOV_RESOURCE and attribute @ism:declassException of ISM_RESOURCE_ELEMENT contains 
        [NATO] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
        @ism:ownerProducer attribute containing [NATO] or the resource level attribute @ism:FGIsourceOpen must contain [NATO].
        
        Human Readable: USA documents marked with a NATO declass exemption must have NATO portions or FGI NATO at the resource level.
    </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE, the current element is the
      ISM_RESOURCE_ELEMENT, and attribute @ism:declassException is specified
      with a value containing the value [NATO], then this rule ensures that some
      element meeting ISM_CONTRIBUTES specifies attribute @ism:ownerProducer
      with a value containing [NATO] or that the resource level @ism:FGIsourceOpen contains [NATO].
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M440"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00317</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00317</xsl:attribute>
            <svrl:text>
        [ISM-ID-00317][Error] If ISM_USGOV_RESOURCE and attribute @ism:declassExemption of ISM_RESOURCE_ELEMENT contains 
        [NATO-AEA] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
        @ism:ownerProducer attribute containing [NATO] and one portion containing @ism:atomicEnergyMarkings.
        
        Human Readable: USA documents marked with a NATO-AEA declass exemption must have at least one NATO portion 
        and one portion that contains Atomic Energy Markings.
    </svrl:text>
            <svrl:text>
      If the document is an ISM_USGOV_RESOURCE, the current element is the
      ISM_RESOURCE_ELEMENT, and attribute @ism:declassExemption is specified
      with a value containing the value [NATO-AEA], then this rule ensures that some
      element meeting ISM_CONTRIBUTES specifies attribute @ism:ownerProducer
      with a value containing [NATO] and @ism:atomicEnergyMarkings.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M441"/>
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
            <xsl:attribute name="id">ISM-ID-00344</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00344</xsl:attribute>
            <svrl:text>
        [ISM-ID-00344][Error] If ISM_USGOV_RESOURCE and there exists a token in @ism:SCIcontrols on the ISM_RESOURCE_ELEMENT
        and no compilation reason then the token must also be specified in the @ism:SCIcontrols attribute 
        on at least one portion.
        
        Human Readable: All SCI controls specified at the resource level must be found in a contributing
        portion of the document unless there is a compilation reason of the exception.
    </svrl:text>
            <svrl:text>
        If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols of
        ISM_RESOURCE_ELEMENT exists and attribute ism:compilationReason does not have a
        value, then this rule ensures that at least one element meeting ISM_CONTRIBUTES specifies attribute
        @ism:SCIcontrols with each value specified on the ISM_RESOURCE_ELEMENT.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M457"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00348</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00348</xsl:attribute>
            <svrl:text>
        [ISM-ID-00348][Error] If ISM_USGOV_RESOURCE and there exists a token in @ism:SARIdentifier on the ISM_RESOURCE_ELEMENT
        and no compilation reason then the token must also be specified in the @ism:SARIdentifer attribute 
        on at least one portion.
        
        Human Readable: All SAR Identifiers specified at the resource level must be found in a contributing
        portion of the document unless there is a compilation reason of the exception.
    </svrl:text>
            <svrl:text>
        If ISM_USGOV_RESOURCE and attribute @ism:SARIdentifier of
        ISM_RESOURCE_ELEMENT exists and attribute @ism:compilationReason does not have a
        value, then this rule ensures that at least one element meeting ISM_CONTRIBUTES specifies attribute
        @ism:SARIdentifier with each value specified on the ISM_RESOURCE_ELEMENT. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M461"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00374</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00374</xsl:attribute>
            <svrl:text> 
        [ISM-ID-00374][Error] If ISM_USGOV_RESOURCE and @ism:nonICmarkings contains 'SSI' on the ISM_RESOURCE_ELEMENT
        with no compilation reason then the token 'SSI' must exist in an @ism:nonICmarkings attribute
        on at least one portion. 
         
        Human Readable: If @ism:nonICmarkings contains 'SSI' at the resource level, it must be found in a contributing
        portion of the document unless there is a compilation reason of the exception.
    </svrl:text>
            <svrl:text>
        If ISM_USGOV_RESOURCE and attribute @ism:nonICmarkings contains 'SSI' 
        on the ISM_RESOURCE_ELEMENT and attribute @ism:compilationReason does not have a
        value, then this rule ensures that at least one element meeting ISM_CONTRIBUTES has attribute
        @ism:nonICmarkings containing 'SSI'.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M483"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00394</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00394</xsl:attribute>
            <svrl:text>
        [ISM-ID-00394][Error] If the ISM_RESOURCE_ELEMENT has the "RAWFISA" dissemination control 
        and no compilation reason, then at least one portion must have the "RAWFISA" dissemination control.
        
        Human Readable: USA documents marked RAWFISA at the resource level must have RAWFISA data.
    </svrl:text>
            <svrl:text>For the ISM_RESOURCE_ELEMENT with attribute @ism:disseminationControls 
        containing the name token "RAWFISA" and no @ism:compilationReason, then some portion of 
        the document must have @ism:disseminationControls containing the "RAWFISA" token.
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M495"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00475</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00475</xsl:attribute>
            <svrl:text> 
        [ISM-ID-00475][Error] If ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, and
        there exists a token in @ism:cuiSpecified on the ISM_RESOURCE_ELEMENT and no compilation reason,
        then the token must also be specified in the @ism:cuiSpecified attribute on at least one
        portion. 
        
        Human Readable: All CUI Specified category markings specified at the resource level
        must be found in a contributing portion of the document unless there is a compilation reason
        of the exception. 
    </svrl:text>
            <svrl:text>
        If ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, and attribute @ism:cuiSpecified
        of ISM_RESOURCE_ELEMENT exists and attribute @ism:compilationReason does not have a value,
        then this rule ensures that at least one element meeting ISM_CONTRIBUTES specifies attribute
        @ism:cuiSpecified with each value specified on the ISM_RESOURCE_ELEMENT. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M519"/>
         <svrl:active-pattern>
            <xsl:attribute name="document">
               <xsl:value-of select="document-uri(/)"/>
            </xsl:attribute>
            <xsl:attribute name="id">ISM-ID-00504</xsl:attribute>
            <xsl:attribute name="name">ISM-ID-00504</xsl:attribute>
            <svrl:text>
        [ISM-ID-00504][Error] If ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, and
        there exists a token in @ism:cuiBasic on the ISM_RESOURCE_ELEMENT and no compilation reason,
        then the token must also be specified in the @ism:cuiBasic attribute on at least one
        portion. 
        
        Human Readable: All CUI Basic category markings specified at the resource level
        must be found in a contributing portion of the document unless there is a compilation reason
        of the exception. 
    </svrl:text>
            <svrl:text>If ISM_USCUI_RESOURCE or ISM_USCUIONLY_RESOURCE, and attribute @ism:cuiBasic
        of ISM_RESOURCE_ELEMENT exists and attribute @ism:compilationReason does not have a value,
        then this rule ensures that at least one element meeting ISM_CONTRIBUTES specifies attribute
        @ism:cuiBasic with each value specified on the ISM_RESOURCE_ELEMENT. 
    </svrl:text>
            <xsl:apply-templates/>
         </svrl:active-pattern>
         <xsl:apply-templates select="/" mode="M544"/>
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

   <!--PATTERN ISM-ID-00056-->


	<!--RULE ISM-ID-00056-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and normalize-space(string(@ism:classification)) = 'U']"
                 priority="1000"
                 mode="M273">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and normalize-space(string(@ism:classification)) = 'U']"
                       id="ISM-ID-00056-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $ele in $partTags satisfies not(util:containsAnyOfTheTokens($ele/@ism:classification, ('C', 'S', 'TS', 'R')))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $ele in $partTags satisfies not(util:containsAnyOfTheTokens($ele/@ism:classification, ('C', 'S', 'TS', 'R')))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> 
            [ISM-ID-00056][Error] If the document is an ISM_USGOV_RESOURCE and
            attribute @ism:classification of ISM_RESOURCE_ELEMENT has a value of [U] then no element meeting
            ISM_CONTRIBUTES in the document may have a @ism:classification attribute of [C], [S], [TS], or [R]. 
            
            Human Readable: USA UNCLASSIFIED documents cannot have portion markings with the
            classification TOP SECRET, SECRET, CONFIDENTIAL, or RESTRICTED data. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M273"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M273"/>
   <xsl:template match="@*|node()" priority="-2" mode="M273">
      <xsl:apply-templates select="*" mode="M273"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00058-->


	<!--RULE ISM-ID-00058-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and normalize-space(string(@ism:classification))='C']"
                 priority="1000"
                 mode="M274">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and normalize-space(string(@ism:classification))='C']"
                       id="ISM-ID-00058-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $ele in $partTags satisfies not(util:containsAnyOfTheTokens($ele/@ism:classification, ('S', 'TS')))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $ele in $partTags satisfies not(util:containsAnyOfTheTokens($ele/@ism:classification, ('S', 'TS')))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
          [ISM-ID-00058][Error] If ISM_USGOV_RESOURCE and attribute @ism:classification of ISM_RESOURCE_ELEMENT 
          has a value of [C] then no element meeting ISM_CONTRIBUTES_USA in the document may have a @ism:classification 
          attribute of [S] or [TS].
          
          Human Readable: USA CONFIDENTIAL documents cannot have TOP SECRET or SECRET data.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M274"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M274"/>
   <xsl:template match="@*|node()" priority="-2" mode="M274">
      <xsl:apply-templates select="*" mode="M274"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00059-->


	<!--RULE ISM-ID-00059-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and normalize-space(string(@ism:classification))='S']"
                 priority="1000"
                 mode="M275">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and normalize-space(string(@ism:classification))='S']"
                       id="ISM-ID-00059-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $ele in $partTags satisfies not(util:containsAnyOfTheTokens($ele/@ism:classification, ('TS')))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $ele in $partTags satisfies not(util:containsAnyOfTheTokens($ele/@ism:classification, ('TS')))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
          [ISM-ID-00059][Error] If ISM_USGOV_RESOURCE and attribute @ism:classification of ISM_RESOURCE_ELEMENT 
          has a value of [S] then no element meeting ISM_CONTRIBUTES_USA in the document may have 
          a @ism:classification attribute of [TS].
          
          Human Readable: USA SECRET documents can't have TOP SECRET data.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M275"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M275"/>
   <xsl:template match="@*|node()" priority="-2" mode="M275">
      <xsl:apply-templates select="*" mode="M275"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00108-->


	<!--RULE NonCompilationDocumentRollup-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:classification, ('TS')) and string-length(normalize-space(@ism:compilationReason)) = 0]"
                 priority="1000"
                 mode="M303">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:classification, ('TS')) and string-length(normalize-space(@ism:compilationReason)) = 0]"
                       id="NonCompilationDocumentRollup-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:classification, ('TS'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:classification, ('TS'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			               <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00108][Error] USA TS documents not using compilation must have TS data.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M303"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M303"/>
   <xsl:template match="@*|node()" priority="-2" mode="M303">
      <xsl:apply-templates select="*" mode="M303"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00109-->


	<!--RULE NonCompilationDocumentRollup-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:classification, ('S')) and string-length(normalize-space(@ism:compilationReason)) = 0]"
                 priority="1000"
                 mode="M304">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:classification, ('S')) and string-length(normalize-space(@ism:compilationReason)) = 0]"
                       id="NonCompilationDocumentRollup-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:classification, ('S'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:classification, ('S'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			               <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00109][Error] USA S documents not using compilation must have S data.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M304"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M304"/>
   <xsl:template match="@*|node()" priority="-2" mode="M304">
      <xsl:apply-templates select="*" mode="M304"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00110-->


	<!--RULE NonCompilationDocumentRollup-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:classification, ('C')) and string-length(normalize-space(@ism:compilationReason)) = 0]"
                 priority="1000"
                 mode="M305">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:classification, ('C')) and string-length(normalize-space(@ism:compilationReason)) = 0]"
                       id="NonCompilationDocumentRollup-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:classification, ('C'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:classification, ('C'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			               <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00110][Error] USA C documents not using compilation must have C data.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M305"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M305"/>
   <xsl:template match="@*|node()" priority="-2" mode="M305">
      <xsl:apply-templates select="*" mode="M305"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00128-->


	<!--RULE DataHasCorrespondingNoticeWithException-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings, ('FRD')) and not(util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings, ('RD')))]"
                 priority="1000"
                 mode="M308">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings, ('FRD')) and not(util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:atomicEnergyMarkings, ('RD')))]"
                       id="DataHasCorrespondingNoticeWithException-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('FRD')) and not($elem/@ism:externalNotice = true()))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ('FRD')) and not($elem/@ism:externalNotice = true()))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			[<xsl:text/>
                  <xsl:value-of select="'ISM-ID-00128'"/>
                  <xsl:text/>][Error] If ISM_USGOV_RESOURCE, any
			element meeting ISM_CONTRIBUTES in the document has the attribute <xsl:text/>
                  <xsl:value-of select="'atomicEnergyMarkings'"/>
                  <xsl:text/> containing [<xsl:text/>
                  <xsl:value-of select="'FRD'"/>
                  <xsl:text/>], then some
			element meeting ISM_CONTRIBUTES in the document MUST have attribute noticeType
			containing [<xsl:text/>
                  <xsl:value-of select="'FRD'"/>
                  <xsl:text/>].</svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M308"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M308"/>
   <xsl:template match="@*|node()" priority="-2" mode="M308">
      <xsl:apply-templates select="*" mode="M308"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00132-->


	<!--RULE ISM-ID-00132-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE  and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RELIDO'))]"
                 priority="1000"
                 mode="M311">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE  and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RELIDO'))]"
                       id="ISM-ID-00132-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $ele in $partTags satisfies if ($ele/@ism:classification[normalize-space()='U'] and not(util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('REL','NF','DISPLAYONLY'))) and not(util:containsAnyOfTheTokens($ele/@ism:nonICmarkings, ('SBU-NF', 'LES-NF')))) then true() else util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('RELIDO'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $ele in $partTags satisfies if ($ele/@ism:classification[normalize-space()='U'] and not(util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('REL','NF','DISPLAYONLY'))) and not(util:containsAnyOfTheTokens($ele/@ism:nonICmarkings, ('SBU-NF', 'LES-NF')))) then true() else util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('RELIDO'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00132][Error] If ISM_USGOV_RESOURCE and the
            ISM_RESOURCE_ELEMENT has the attribute @ism:disseminationControls containing [RELIDO] then every
            element meeting ISM_CONTRIBUTES_CLASSIFIED in the document must have the attribute
            @ism:disseminationControls containing [RELIDO]. 
            
            Human Readable: USA documents having RELIDO at the resource level must have every classified portion 
            having RELIDO and on any U portions that have explicit Release specified must have RELIDO. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M311"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M311"/>
   <xsl:template match="@*|node()" priority="-2" mode="M311">
      <xsl:apply-templates select="*" mode="M311"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00154-->


	<!--RULE NonCompilationDocumentRollup-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('FOUO')) and string-length(normalize-space(@ism:compilationReason)) = 0]"
                 priority="1000"
                 mode="M330">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('FOUO')) and string-length(normalize-space(@ism:compilationReason)) = 0]"
                       id="NonCompilationDocumentRollup-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('FOUO'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:disseminationControls, ('FOUO'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
			               <xsl:text/>
                  <xsl:value-of select="'[ISM-ID-00154][Error] USA FOUO documents not using compilation must have FOUO data.'"/>
                  <xsl:text/>
               </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M330"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M330"/>
   <xsl:template match="@*|node()" priority="-2" mode="M330">
      <xsl:apply-templates select="*" mode="M330"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00219-->


	<!--RULE ISM-ID-00219-R1-->
<xsl:template match="*[not(generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)) and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:ownerProducer, ('FGI'))]"
                 priority="1000"
                 mode="M373">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[not(generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)) and util:contributesToRollup(.) and util:containsAnyOfTheTokens(@ism:ownerProducer, ('FGI'))]"
                       id="ISM-ID-00219-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyOfTheTokens(@ism:FGIsourceProtected, ('FGI'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyOfTheTokens(@ism:FGIsourceProtected, ('FGI'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00219][Error] If element meets ISM_CONTRIBUTES and attribute
            @ism:ownerProducer contains the token [FGI], then attribute 
            @ism:FGIsourceProtected must have a value containing the token [FGI].
            
            Human Readable: Any non-resource element that contributes to the 
            document's banner roll-up and has FOREIGN GOVERNMENT INFORMATION (FGI)
            must also specify attribute FGIsourceProtected with token FGI.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M373"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M373"/>
   <xsl:template match="@*|node()" priority="-2" mode="M373">
      <xsl:apply-templates select="*" mode="M373"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00228-->


	<!--RULE ISM-ID-00228-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('FRD'))]"
                 priority="1000"
                 mode="M377">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('FRD'))]"
                       id="ISM-ID-00228-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partAtomicEnergyMarkings_tok,'FRD')&gt;0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partAtomicEnergyMarkings_tok,'FRD')&gt;0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00228][Error] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings of ISM_RESOURCE_ELEMENT contains 
            [FRD] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
            @ism:atomicEnergyMarking attribute containing [FRD].
            
            Human Readable: USA documents marked FRD at the resource level must have FRD data.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M377"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M377"/>
   <xsl:template match="@*|node()" priority="-2" mode="M377">
      <xsl:apply-templates select="*" mode="M377"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00229-->


	<!--RULE ISM-ID-00229-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD'))]"
                 priority="1000"
                 mode="M378">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD'))]"
                       id="ISM-ID-00229-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partAtomicEnergyMarkings_tok,'RD') &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partAtomicEnergyMarkings_tok,'RD') &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00229][Error] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings of ISM_RESOURCE_ELEMENT contains 
            [RD] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
            @ism:atomicEnergyMarking attribute containing [RD].
            
            Human Readable: USA documents marked RD at the resource level must have RD data.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M378"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M378"/>
   <xsl:template match="@*|node()" priority="-2" mode="M378">
      <xsl:apply-templates select="*" mode="M378"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00230-->


	<!--RULE ISM-ID-00230-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                 priority="1000"
                 mode="M379">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                       id="ISM-ID-00230-R1"/>
      <xsl:variable name="matchingTokens"
                    select="for $token in tokenize(normalize-space(string(@ism:atomicEnergyMarkings)), ' ') return if(matches($token,'^FRD-SG-[1-9][0-9]?$')) then $token else null"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $token in $matchingTokens satisfies index-of($partAtomicEnergyMarkings_tok, $token) &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $token in $matchingTokens satisfies index-of($partAtomicEnergyMarkings_tok, $token) &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00230][Error] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings of ISM_RESOURCE_ELEMENT contains 
            [FRD-SG-##] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
            @ism:atomicEnergyMarking attribute containing the same [FRD-SG-##].
            
            Human Readable: USA documents marked FRD-SG-## at the resource level must have FRD-SG-## data, where ## is the same.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M379"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M379"/>
   <xsl:template match="@*|node()" priority="-2" mode="M379">
      <xsl:apply-templates select="*" mode="M379"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00231-->


	<!--RULE ISM-ID-00231-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                 priority="1000"
                 mode="M380">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]"
                       id="ISM-ID-00231-R1"/>
      <xsl:variable name="matchingTokens"
                    select="for $token in tokenize(normalize-space(string(@ism:atomicEnergyMarkings)), ' ') return if(matches($token,'^RD-SG-[1-9][0-9]?$')) then $token else null"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $token in $matchingTokens satisfies (index-of($partAtomicEnergyMarkings_tok, $token) &gt; 0 or index-of($partAtomicEnergyMarkings_tok, concat('F', $token)) &gt; 0)"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $token in $matchingTokens satisfies (index-of($partAtomicEnergyMarkings_tok, $token) &gt; 0 or index-of($partAtomicEnergyMarkings_tok, concat('F', $token)) &gt; 0)">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
          [ISM-ID-00231][Error] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings of ISM_RESOURCE_ELEMENT contains 
          [RD-SG-##] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
          @ism:atomicEnergyMarking attribute containing the same [RD-SG-##].
          
          Human Readable: USA documents marked RD-SG-## at the resource level must have RD-SG-## or FRD-SG-## data, where ## is the same.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M380"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M380"/>
   <xsl:template match="@*|node()" priority="-2" mode="M380">
      <xsl:apply-templates select="*" mode="M380"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00252-->


	<!--RULE ISM-ID-00252-R1-->
<xsl:template match="*[index-of(tokenize(normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:disseminationControls)), ' '),'RELIDO') &gt; 0 and @ism:nonICmarkings]"
                 priority="1000"
                 mode="M387">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[index-of(tokenize(normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:disseminationControls)), ' '),'RELIDO') &gt; 0 and @ism:nonICmarkings]"
                       id="ISM-ID-00252-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="not(util:containsAnyTokenMatching(@ism:nonICmarkings, 'NNPI'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="not(util:containsAnyTokenMatching(@ism:nonICmarkings, 'NNPI'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00252][Error] If ISM_RESOURCE_ELEMENT specifies the attribute
            @ism:disseminationControls with a value containing the token [RELIDO], 
            then attribute @ism:nonICmarkings must not be specified with a value containing 
            the token [NNPI]. 
            
            Human Readable: NNPI tokens are not valid for documents that have
            RELIDO at the resource level.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M387"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M387"/>
   <xsl:template match="@*|node()" priority="-2" mode="M387">
      <xsl:apply-templates select="*" mode="M387"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00303-->


	<!--RULE ISM-ID-00303-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC-USGOV'))]"
                 priority="1000"
                 mode="M436">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC-USGOV'))]"
                       id="ISM-ID-00303-R1"/>
      <xsl:variable name="portionsWithOC"
                    select="for $portion in $partTags return if($portion[util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))]) then $portion else null"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="every $portionWithOC in $portionsWithOC satisfies $portionWithOC[util:containsAnyOfTheTokens(@ism:disseminationControls, 'OC-USGOV')]"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="every $portionWithOC in $portionsWithOC satisfies $portionWithOC[util:containsAnyOfTheTokens(@ism:disseminationControls, 'OC-USGOV')]">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00303][Error] If ISM_USGOV_RESOURCE and the document contains attribute 
            @ism:disseminationControls with name token [OC-USGOV] in the banner, then 
            all [OC] portions must also contain [OC-USGOV].
            
            Human Readable: A USA document with OC-USGOV dissemination in the banner
            must also contain OC-USGOV in any OC portions.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M436"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M436"/>
   <xsl:template match="@*|node()" priority="-2" mode="M436">
      <xsl:apply-templates select="*" mode="M436"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00316-->


	<!--RULE ISM-ID-00316-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:declassException, ('NATO'))]"
                 priority="1000"
                 mode="M440">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:declassException, ('NATO'))]"
                       id="ISM-ID-00316-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyTokenMatching(string-join($partOwnerProducer_tok,' '), ('^NATO:?'))             or util:containsAnyTokenMatching(string-join($bannerFGIsourceOpen_tok,' '), ('^NATO:?'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyTokenMatching(string-join($partOwnerProducer_tok,' '), ('^NATO:?')) or util:containsAnyTokenMatching(string-join($bannerFGIsourceOpen_tok,' '), ('^NATO:?'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00316][Error] If ISM_USGOV_RESOURCE and attribute @ism:declassException of ISM_RESOURCE_ELEMENT contains 
            [NATO] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
            @ism:ownerProducer attribute containing [NATO] or the resource level attribute @ism:FGIsourceOpen must contain [NATO].
            
            Human Readable: USA documents marked with a NATO declass exemption must have NATO portions or FGI NATO at the resource level.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M440"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M440"/>
   <xsl:template match="@*|node()" priority="-2" mode="M440">
      <xsl:apply-templates select="*" mode="M440"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00317-->


	<!--RULE ISM-ID-00317-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:declassException, ('NATO-AEA'))]"
                 priority="1000"
                 mode="M441">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:declassException, ('NATO-AEA'))]"
                       id="ISM-ID-00317-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="util:containsAnyTokenMatching(string-join($partOwnerProducer_tok, ' '), ('NATO:?')) and count($partAtomicEnergyMarkings_tok)&gt;0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="util:containsAnyTokenMatching(string-join($partOwnerProducer_tok, ' '), ('NATO:?')) and count($partAtomicEnergyMarkings_tok)&gt;0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00317][Error] If ISM_USGOV_RESOURCE and attribute @ism:declassExemption of ISM_RESOURCE_ELEMENT contains 
            [NATO-AEA] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
            @ism:ownerProducer attribute containing [NATO] and one portion containing @ism:atomicEnergyMarkings.
            
            Human Readable: USA documents marked with a NATO-AEA declass exemption must have at least one NATO portion 
            and one portion that contains Atomic Energy Markings.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M441"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M441"/>
   <xsl:template match="@*|node()" priority="-2" mode="M441">
      <xsl:apply-templates select="*" mode="M441"/>
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

   <!--PATTERN ISM-ID-00344-->


	<!--RULE ISM-ID-00344-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:SCIcontrols and string-length(normalize-space(@ism:compilationReason)) = 0]"
                 priority="1000"
                 mode="M457">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:SCIcontrols and string-length(normalize-space(@ism:compilationReason)) = 0]"
                       id="ISM-ID-00344-R1"/>
      <xsl:variable name="missingSCI"
                    select="for $token in tokenize(@ism:SCIcontrols, ' ') return if (index-of(distinct-values($partSCIcontrols), $token) &gt; 0) then null else $token"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count($missingSCI)=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="count($missingSCI)=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00344][Error] All SCI controls specified at the resource level must be found in a contributing
            portion of the document unless there is a compilation reason of the exception. The following tokens 
            were found to be missing from the portions: <xsl:text/>
                  <xsl:value-of select="string-join($missingSCI, ', ')"/>
                  <xsl:text/>.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M457"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M457"/>
   <xsl:template match="@*|node()" priority="-2" mode="M457">
      <xsl:apply-templates select="*" mode="M457"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00348-->


	<!--RULE ISM-ID-00348-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:SARIdentifier  and string-length(normalize-space(@ism:compilationReason)) = 0]"
                 priority="1000"
                 mode="M461">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:SARIdentifier  and string-length(normalize-space(@ism:compilationReason)) = 0]"
                       id="ISM-ID-00348-R1"/>
      <xsl:variable name="missingSAR"
                    select="for $token in tokenize(@ism:SARIdentifier, ' ') return if (index-of(distinct-values($partSARIdentifier), $token) &gt; 0) then null else $token"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count($missingSAR)=0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl" test="count($missingSAR)=0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00348][Error] All SAR Identifiers specified at the resource level must be found in a contributing
            portion of the document unless there is a compilation reason of the exception. The following tokens 
            were found to be missing from the portions: <xsl:text/>
                  <xsl:value-of select="string-join($missingSAR, ', ')"/>
                  <xsl:text/>.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M461"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M461"/>
   <xsl:template match="@*|node()" priority="-2" mode="M461">
      <xsl:apply-templates select="*" mode="M461"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00374-->


	<!--RULE ISM-ID-00374-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('SSI')) and string-length(normalize-space(@ism:compilationReason)) = 0]"
                 priority="1000"
                 mode="M483">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('SSI')) and string-length(normalize-space(@ism:compilationReason)) = 0]"
                       id="ISM-ID-00374-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:nonICmarkings, ('SSI'))"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="some $ele in $partTags satisfies util:containsAnyOfTheTokens($ele/@ism:nonICmarkings, ('SSI'))">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00374][Error] If ISM_USGOV_RESOURCE and @ism:nonICmarkings contains 'SSI' on the ISM_RESOURCE_ELEMENT
            with no compilation reason then the token 'SSI' must exist in an @ism:nonICmarkings attribute
            on at least one portion. 
            
            Human Readable: If @ism:nonICmarkings contains 'SSI' at the resource level, it must be found in a contributing
            portion of the document unless there is a compilation reason of the exception.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M483"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M483"/>
   <xsl:template match="@*|node()" priority="-2" mode="M483">
      <xsl:apply-templates select="*" mode="M483"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00394-->


	<!--RULE ISM-ID-00394-R1-->
<xsl:template match="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RAWFISA')) and not(@ism:compilationReason)]"
                 priority="1000"
                 mode="M495">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RAWFISA')) and not(@ism:compilationReason)]"
                       id="ISM-ID-00394-R1"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="index-of($partDisseminationControls_tok, 'RAWFISA') &gt; 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="index-of($partDisseminationControls_tok, 'RAWFISA') &gt; 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text>
            [ISM-ID-00394][Error] If the ISM_RESOURCE_ELEMENT has the "RAWFISA" dissemination control 
            and no compilation reason, then at least one portion must have the "RAWFISA" dissemination control.
            
            Human Readable: USA documents marked RAWFISA at the resource level must have RAWFISA data.
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M495"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M495"/>
   <xsl:template match="@*|node()" priority="-2" mode="M495">
      <xsl:apply-templates select="*" mode="M495"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00475-->


	<!--RULE ISM-ID-00475-R1-->
<xsl:template match="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:cuiSpecified and string-length(normalize-space(@ism:compilationReason)) = 0]"
                 priority="1000"
                 mode="M519">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:cuiSpecified and string-length(normalize-space(@ism:compilationReason)) = 0]"
                       id="ISM-ID-00475-R1"/>
      <xsl:variable name="missingCuiSpecified"
                    select="for $token in tokenize(@ism:cuiSpecified, ' ') return if (index-of(distinct-values($partCuiSpecified), $token) &gt; 0) then null else $token"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count($missingCuiSpecified) = 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count($missingCuiSpecified) = 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> 
            [ISM-ID-00475][Error] All CUI Specified category markings specified at the resource level 
            must be found in a contributing portion of the document unless there is a compilation reason of the exception. 
            The following tokens were found to be missing from the portions: <xsl:text/>
                  <xsl:value-of select="string-join($missingCuiSpecified, ', ')"/>
                  <xsl:text/>. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M519"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M519"/>
   <xsl:template match="@*|node()" priority="-2" mode="M519">
      <xsl:apply-templates select="*" mode="M519"/>
   </xsl:template>

   <!--PATTERN ISM-ID-00504-->


	<!--RULE ISM-ID-00504-R1-->
<xsl:template match="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:cuiBasic and string-length(normalize-space(@ism:compilationReason)) = 0]"
                 priority="1000"
                 mode="M544">
      <svrl:fired-rule xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                       context="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and @ism:cuiBasic and string-length(normalize-space(@ism:compilationReason)) = 0]"
                       id="ISM-ID-00504-R1"/>
      <xsl:variable name="missingCuiBasic"
                    select="for $token in tokenize(@ism:cuiBasic, ' ') return if (index-of(distinct-values($partCuiBasic), $token) &gt; 0) then null else $token"/>

		    <!--ASSERT error-->
<xsl:choose>
         <xsl:when test="count($missingCuiBasic) = 0"/>
         <xsl:otherwise>
            <svrl:failed-assert xmlns:svrl="http://purl.oclc.org/dsdl/svrl"
                                test="count($missingCuiBasic) = 0">
               <xsl:attribute name="flag">error</xsl:attribute>
               <xsl:attribute name="role">error</xsl:attribute>
               <xsl:attribute name="location">
                  <xsl:apply-templates select="." mode="schematron-select-full-path"/>
               </xsl:attribute>
               <svrl:text> 
            [ISM-ID-00504][Error] All CUI Basic category markings specified at the resource level 
            must be found in a contributing portion of the document unless there is a compilation reason of the exception. 
            The following tokens were found to be missing from the portions: <xsl:text/>
                  <xsl:value-of select="string-join($missingCuiBasic, ', ')"/>
                  <xsl:text/>. 
        </svrl:text>
            </svrl:failed-assert>
         </xsl:otherwise>
      </xsl:choose>
      <xsl:apply-templates select="*" mode="M544"/>
   </xsl:template>
   <xsl:template match="text()" priority="-1" mode="M544"/>
   <xsl:template match="@*|node()" priority="-2" mode="M544">
      <xsl:apply-templates select="*" mode="M544"/>
   </xsl:template>
</xsl:stylesheet>
<!--UNCLASSIFIED-->
