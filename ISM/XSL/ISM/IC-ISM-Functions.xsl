<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xd="http://www.oxygenxml.com/ns/doc/xsl"
    xmlns:ism-func="urn:us:gov:ic:ism:functions" xmlns:cve="urn:us:gov:ic:cve"
    xmlns:catt="urn:us:gov:ic:taxonomy:catt:tetragraph"
    exclude-result-prefixes="xs xd" version="2.0">
    <xd:doc scope="stylesheet">
        <xd:desc>
            <xd:p><xd:b>Created on:</xd:b> Sep 22, 2019</xd:p>
            <xd:p><xd:b>Author:</xd:b>IC-CIO</xd:p>
            <xd:p/>
        </xd:desc>
    </xd:doc>

    <xsl:param name="warn-missing-classif" select="'MISSING CLASSIFICATION MARKING'"/>
    <xsl:param name="warn-parse-classif" select="'UNABLE TO DETERMINE CLASSIFICATION MARKING'"/>
    <xsl:param name="warn-parse-ownerproducer"
        select="concat($warn-parse-classif, ' - MISSING OWNER/PRODUCER')"/>
    <xsl:param name="warn-parse-relto" select="'UNABLE TO DETERMINE RELEASABILITY'"/>
    <xsl:param name="warn-parse-displayonly" select="'UNABLE TO DETERMINE DISPLAY ONLY'"/>
    <xsl:param name="warn-parse-eyes" select="'UNABLE TO DETERMINE EYES ONLY MARKINGS'"/>

    <xsl:param name="pathToISMCATCVE" select="'../../CVE/ISMCAT/'"/>
    <xsl:param name="pathToISMCVE" select="'../../CVE/ISM/'"/>
    
    <xsl:param name="pathToISMCATTaxonomy" select="'../../Taxonomy/ISMCAT/'"/>

    <xsl:variable name="FGIOpenCVE"
        select="document(concat($pathToISMCATCVE, 'CVEnumISMCATFGIOpen.xml'))//cve:CVE/cve:Enumeration"/>
    <xsl:variable name="FGIProtectedCVE"
        select="document(concat($pathToISMCATCVE, 'CVEnumISMCATFGIProtected.xml'))//cve:CVE/cve:Enumeration"/>
    <xsl:variable name="OwnerProducerCVE"
        select="document(concat($pathToISMCATCVE, 'CVEnumISMCATOwnerProducer.xml'))//cve:CVE/cve:Enumeration"/>
    <xsl:variable name="RelCVE"
        select="document(concat($pathToISMCATCVE, 'CVEnumISMCATRelTo.xml'))//cve:CVE/cve:Enumeration"/>
    <xsl:variable name="disseminationControlsIcrmCVE"
        select="document(concat($pathToISMCVE, 'CVEnumISMDissemIcrm.xml'))//cve:CVE/cve:Enumeration"/>
    <xsl:variable name="disseminationControlsCUICVE"
        select="document(concat($pathToISMCVE, 'CVEnumISMDissemCui.xml'))//cve:CVE/cve:Enumeration"/>
    <xsl:variable name="disseminationControlsCommingledCVE"
        select="document(concat($pathToISMCVE, 'CVEnumISMDissemCommingled.xml'))//cve:CVE/cve:Enumeration"/>
    
    <!-- Only used by Rollup 2021-02-23 When Rollup embraces CUI should go away -->
    <xsl:variable name="disseminationControlsCVE"
        select="document(concat($pathToISMCVE, 'CVEnumISMDissem.xml'))//cve:CVE/cve:Enumeration"/>
        
    <xsl:variable name="NonICControlsCVE"
        select="document(concat($pathToISMCVE, 'CVEnumISMNonIC.xml'))//cve:CVE/cve:Enumeration"/>
    <xsl:variable name="AtomicEnergyCVE"
        select="document(concat($pathToISMCVE, 'CVEnumISMAtomicEnergyMarkings.xml'))//cve:CVE/cve:Enumeration"/>
    <xsl:variable name="nonICCVE"
        select="document(concat($pathToISMCVE, 'CVEnumISMNonIC.xml'))//cve:CVE/cve:Enumeration"/>
    <xsl:variable name="cuiBasic"
        select="document(concat($pathToISMCVE, 'CVEnumISMCUIBasic.xml'))//cve:CVE/cve:Enumeration"/>
    <xsl:variable name="cuiSpecified"
        select="document(concat($pathToISMCVE, 'CVEnumISMCUISpecified.xml'))//cve:CVE/cve:Enumeration"/>
    <xsl:variable name="secondBannerLineCVE"
        select="document(concat($pathToISMCVE, 'CVEnumISMSecondBannerLine.xml'))//cve:CVE/cve:Enumeration"/>

    <!-- Regex -->
    <xsl:variable name="ACCMRegex" select="'^ACCM-[A-Z0-9\-_]{1,61}$'"/>

    <!-- nonACCM values left and right of ACCM values defined in CVEnumISMNonIC.xml -->
    <xsl:variable name="nonACCMLeftSet" select="'DS'"/>
    <xsl:variable name="nonACCMRightSet" select="'XD,ND,SBU,SBU-NF,LES,LES-NF,SSI,NNPI'"/>
    <xsl:variable name="nonACCMLeftSetTok" select="tokenize($nonACCMLeftSet, ',')"/>
    <xsl:variable name="nonACCMRightSetTok" select="tokenize($nonACCMRightSet, ',')"/>
    
    <xsl:variable name="catt"
        select="document(concat($pathToISMCATTaxonomy,'TetragraphTaxonomyDenormalized.xml'))"/>
    
    <xsl:variable name="cattMappings" select="$catt//catt:Tetragraph"/>
    
    <xsl:variable name="decomposableTetraElems"
        select="$cattMappings[@decomposable[. = 'Yes' or . = 'NA']]"/>
    
    <xsl:variable name="decomposableTetras"
        select="$decomposableTetraElems/catt:TetraToken/text()"/>

    <xsl:function name="ism-func:classStringForClass">
        <xsl:param name="class"/>
        <xsl:choose>
            <xsl:when test="$class = 'TS'">TOP SECRET</xsl:when>
            <xsl:when test="$class = 'S'">SECRET</xsl:when>
            <xsl:when test="$class = 'C'">CONFIDENTIAL</xsl:when>
            <xsl:when test="$class = 'R'">RESTRICTED</xsl:when>
            <xsl:when test="$class = 'U'">UNCLASSIFIED</xsl:when>
            <xsl:otherwise>
                <xsl:value-of select="$warn-parse-classif"/>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:function>

    <!-- ********************************************************** -->
    <!-- A routine for processing SCIcontrols name tokens -->
    <!-- ********************************************************** -->
    <xsl:function name="ism-func:get.sci">
        <xsl:param name="all"/>

        <!-- Loop over all the SCI tokens -->
        <xsl:for-each select="tokenize($all, ' ')">
            <xsl:variable name="tokenizedSCIToken" select="tokenize(current(), '-')"/>
            <xsl:variable name="compartmentLevelCount" select="count($tokenizedSCIToken) - 1"/>
            <xsl:choose>
                <!-- Not the first SCI and has no compartment/subcompartments add a / -->
                <xsl:when test="$compartmentLevelCount = 0 and not(position() = 1)">
                    <xsl:text>/</xsl:text>
                </xsl:when>
                <!-- A compartment add a - -->
                <xsl:when test="$compartmentLevelCount = 1">
                    <xsl:text>-</xsl:text>
                </xsl:when>
                <!-- A subcompartment add a space -->
                <xsl:when test="$compartmentLevelCount = 2">
                    <xsl:text> </xsl:text>
                </xsl:when>
            </xsl:choose>
            <xsl:value-of select="$tokenizedSCIToken[last()]"/>
        </xsl:for-each>
    </xsl:function>

    <xsl:function name="ism-func:sciVal">
        <xsl:param name="sci"/>
        <xsl:param name="nonUSControls"/>

        <xsl:if test="$sci != ''">
            <xsl:text>//</xsl:text>
            <xsl:value-of select="ism-func:get.sci($sci)"/>

            <xsl:if test="$nonUSControls and contains($nonUSControls, 'BALK')">
                <xsl:text>/BALK</xsl:text>
            </xsl:if>

            <xsl:if test="$nonUSControls and contains($nonUSControls, 'BOHEMIA')">
                <xsl:text>/BOHEMIA</xsl:text>
            </xsl:if>
        </xsl:if>
    </xsl:function>

    <xsl:function name="ism-func:AEAVal">
        <xsl:param name="atomicEnergyMarking"/>
        <xsl:param name="nonUSControlsLocal"/>
        <xsl:param name="banner"/>
        <xsl:if test="normalize-space($atomicEnergyMarking) != ''">
            <xsl:text>//</xsl:text>
            <xsl:value-of select="ism-func:getAEA($atomicEnergyMarking, $banner)"/>
            <xsl:if test="$nonUSControlsLocal and contains($nonUSControlsLocal, 'ATOMAL')">
                <xsl:text>/ATOMAL</xsl:text>
            </xsl:if>
        </xsl:if>
    </xsl:function>

    <xsl:function name="ism-func:getAEA">
        <xsl:param name="all"/>
        <xsl:param name="banner"/>
        <xsl:variable name="tokenizedAEA" select="tokenize($all, ' ')"/>
        <xsl:for-each select="$tokenizedAEA">
            <xsl:variable name="tokenizedAEAToken" select="tokenize(current(), '-')"/>
            <xsl:variable name="compartmentLevelCount" select="count($tokenizedAEAToken) - 1"/>
            <xsl:variable name="currentPosition" select="position()"/>
            <xsl:choose>
                <!-- Not the first AEA and has no compartment/subcompartments add a / -->
                <xsl:when test="$compartmentLevelCount = 0 and not(position() = 1)">
                    <xsl:text>/</xsl:text>
                </xsl:when>
                <!-- A compartment add a - -->
                <xsl:when test="$compartmentLevelCount = 1">
                    <xsl:text>-</xsl:text>
                </xsl:when>
                <!-- AEA does not really have subcompartments SIGMA's look like they are subs so add space if not the first one. -->
                <xsl:when test="$compartmentLevelCount = 2">
                    <!-- First Sigma add -SG then move on -->
                    <xsl:if
                        test="not(($tokenizedAEAToken[last() - 1] = 'SG') and (tokenize($tokenizedAEA[$currentPosition - 1], '-')[last() - 1] = 'SG'))">
                        <xsl:text>-</xsl:text>
                        <xsl:choose>
                            <xsl:when test="$banner">SIGMA</xsl:when>
                            <xsl:otherwise>
                                <xsl:value-of select="$tokenizedAEAToken[last() - 1]"/>
                            </xsl:otherwise>
                        </xsl:choose>
                    </xsl:if>
                    <xsl:text> </xsl:text>
                </xsl:when>
            </xsl:choose>
            <xsl:choose>
                <xsl:when test="$banner and $tokenizedAEAToken[last()] = 'DCNI'">DOD UCNI</xsl:when>
                <xsl:when test="$banner and $tokenizedAEAToken[last()] = 'UCNI'">DOE UCNI</xsl:when>
                <xsl:otherwise>
                    <xsl:value-of select="$tokenizedAEAToken[last()]"/>
                </xsl:otherwise>
            </xsl:choose>
        </xsl:for-each>
    </xsl:function>

    <xsl:function name="ism-func:get.sar.name">
        <xsl:param name="name"/>
        <xsl:sequence select="ism-func:get.sar.name($name, 'yes')"/>
    </xsl:function>

    <!-- **************************************** -->
    <!-- Full name conversion for SAR name token -->
    <!-- **************************************** -->
    <xsl:function name="ism-func:get.sar.name">
        <xsl:param name="name"/>
        <xsl:param name="abbreviate"/>
        <!-- *********************************************************************** -->
        <!-- Set this abbreviate to "yes" to use the program identifier abbreviations. -->
        <!-- Otherwise the program identifiers will be used.                         -->
        <!-- *********************************************************************** -->
        <xsl:variable name="SAR-val">
            <xsl:choose>
                <xsl:when test="substring-after($name, 'SAR-') = ''">
                    <xsl:value-of select="concat('SAR-', $name)"/>
                </xsl:when>
                <xsl:otherwise>
                    <xsl:value-of select="substring-after($name, 'SAR-')"/>
                </xsl:otherwise>
            </xsl:choose>
        </xsl:variable>

        <xsl:choose>
            <!-- ********************************************** -->
            <!-- The actual SAR program identifiers and program -->
            <!-- identifier abbreviations should be substituted -->
            <!-- for the placeholders here.                     -->
            <!-- ********************************************** -->
            <xsl:when test="$abbreviate = 'yes'">
                <xsl:value-of select="ism-func:translateSARname($name)"/>
            </xsl:when>
            <xsl:otherwise>
                <xsl:choose>
                    <xsl:when test="$name = 'ABC'">ALPHA BRAVO CHARLIE</xsl:when>
                    <xsl:when test="$name = 'DEF'">DELTA ECHO FOX</xsl:when>
                    <xsl:when test="$name = 'GHI'">GULF HOTEL INDIGO</xsl:when>
                    <xsl:otherwise>
                        <xsl:value-of select="ism-func:translateSARname($name)"/>
                    </xsl:otherwise>
                </xsl:choose>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:function>
    
    <xd:doc>
        <xd:desc>A function that takes a SAR value of the form SAROwner:SARmarking, and extracts the SARmarking.
            The function also replaces a single underscore _ with a space, and replaces a double underscore __ 
            with a single underscore.  The logic uses a temporary replacement of double underscore with double tilde ~</xd:desc>
        <xd:param name="name"/>
    </xd:doc>
    <xsl:function name="ism-func:translateSARname">
        <xsl:param name="name"/>
        <xsl:choose>
            <xsl:when test="contains($name,'_') and not(contains($name,'__'))">
                <xsl:value-of select="substring-after(replace($name, '_', ' '),':')"/>
            </xsl:when>
            <xsl:when test="contains($name,'__') and not(contains($name,'_'))">
                <xsl:value-of select="substring-after(replace($name, '__', '_'),':')"/>
            </xsl:when>
            <xsl:when test="contains($name,'_') and contains($name,'__')">
                <xsl:variable name="doubleunderscoresstep1" select="replace($name,'__','~~')"/>
                <xsl:variable name="singleunderscore" select="replace($doubleunderscoresstep1,'_',' ')"/>
                <xsl:value-of select="substring-after(replace($singleunderscore,'~~','_'),':')"/>
            </xsl:when>
            <xsl:otherwise>                
                <xsl:value-of select="substring-after($name,':')"/>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:function>

    <xsl:function name="ism-func:get.secondBannerLine.name">
        <xsl:param name="token"/>
        <xsl:param name="HVCO"/>
        <xsl:choose>
            <xsl:when test="normalize-space($token) = 'HVCO'">
                <xsl:value-of select="'HANDLE VIA '"/>
                <xsl:value-of select="$HVCO"/>
                <xsl:value-of select="' CHANNELS ONLY'"/>
            </xsl:when>
            <xsl:otherwise>
                <xsl:value-of
                    select="$secondBannerLineCVE//cve:Term[./cve:Value = $token]/cve:Description"/>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:function>

    <xsl:function name="ism-func:get.relOrDisplayString">
        <xsl:param name="countryString"/>
        <xsl:value-of select="ism-func:get.relOrDisplayString($countryString, ', ')"/>
    </xsl:function>

    <xsl:function name="ism-func:get.eyesString">
        <xsl:param name="countryString"/>
        <xsl:value-of select="ism-func:get.relOrDisplayString($countryString, '/')"/>
    </xsl:function>

    <xsl:function name="ism-func:get.relOrDisplayString">
        <xsl:param name="countryString"/>
        <xsl:param name="delimiter"/>
        <xsl:variable name="countryStringWithDelimiters">
            <xsl:value-of select="string-join(tokenize($countryString, ' '), $delimiter)"/>
        </xsl:variable>
        <!-- Deal with NATO extensions like NATO:PFP or NATO:PARTNERSHIP_FOR_PEACE-->
        <xsl:value-of select="translate($countryStringWithDelimiters, '_:', '  ')"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>A routine for processing cuiBasic name token values</xd:desc>
        <xd:param name="all"/>
    </xd:doc>
    <xsl:function name="ism-func:get.cuiBasic">
        <xsl:param name="all"/>
        <xsl:variable name="tokenizedCuiBasic" select="tokenize($all, ' ')"/>
        <xsl:for-each select="$tokenizedCuiBasic">
            <xsl:value-of select="current()"/>
            <!-- Add a trailing / for all but the last cuiBasic marking. -->
            <xsl:if test="position() != last()">
                <xsl:text>/</xsl:text>
            </xsl:if>
        </xsl:for-each>
    </xsl:function>

    <xd:doc>
        <xd:desc>A routine for processing cuiSpecified name token values</xd:desc>
        <xd:param name="all"/>
    </xd:doc>
    <xsl:function name="ism-func:get.cuiSpecified">
        <xsl:param name="all"/>
        <xsl:variable name="tokenizedCuiSpecified" select="tokenize($all, ' ')"/>
        <xsl:for-each select="$tokenizedCuiSpecified">
            <xsl:value-of select="concat('SP-', current())"/>
            <!-- Add a trailing / for all but the last cuiSpecified marking. -->
            <xsl:if test="position() != last()">
                <xsl:text>/</xsl:text>
            </xsl:if>
        </xsl:for-each>
    </xsl:function>
    
    <xd:doc>
        <xd:desc>A routine for processing nonIC name token values in banners and portion marks. For
            banners, the BannerMapping.xml is passed with the banner and portion marking values for
            markings with different banner and portion marks. For portion marks, an empty node set
            is passed.</xd:desc>
        <xd:param name="all"/>
    </xd:doc>
    <xsl:function name="ism-func:get.nonic">
        <xsl:param name="all"/>
        <xsl:param name="DissemLookup"/>
        <xsl:variable name="tokenizedNonic" select="tokenize($all, ' ')"/>
        <xsl:variable name="firstACCMValue" select="$tokenizedNonic[starts-with(., 'ACCM-')][1]"/>
        <xsl:for-each select="$tokenizedNonic">
            <xsl:choose>
                <xsl:when test="$DissemLookup//BannerMap[@portion = current()]">
                    <xsl:value-of select="$DissemLookup//BannerMap[@portion = current()]/text()"/>
                </xsl:when>
                <xsl:when test="starts-with(current(), 'ACCM-')">
                    <!-- Remove ACCM prefix from ACCM tokens -->
                    <xsl:variable name="prefixlessACCM" select="substring-after(current(), 'ACCM-')"/>
                    <!-- Replace '_' with ' ' -->
                    <xsl:if test="current() = $firstACCMValue">
                        <xsl:text>ACCM-</xsl:text>
                    </xsl:if>
                    <xsl:value-of select="translate($prefixlessACCM, '_', ' ')"/>
                </xsl:when>
                <xsl:otherwise>
                    <xsl:value-of select="current()"/>
                </xsl:otherwise>
            </xsl:choose>
            <!-- Add a trailing / for all but the last non-ic dissem control. -->
            <xsl:if test="position() != last()">
                <xsl:text>/</xsl:text>
            </xsl:if>
        </xsl:for-each>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Returns a sort position for a given value passed based in the position in a given
                CVE</xd:p>
        </xd:desc>
        <xd:param name="value"/>
        <xd:param name="cveToSortWith"/>
    </xd:doc>
    <xsl:function name="ism-func:cveSortOrder" as="xs:integer">
        <xsl:param name="value"/>
        <xsl:param name="cveToSortWith" as="node()*"/>
        <xsl:variable name="returnValue" as="xs:integer">
            <xsl:value-of
                select="count($cveToSortWith//cve:Term[$value = cve:Value]/preceding-sibling::*)"/>
        </xsl:variable>
        <xsl:value-of select="$returnValue"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Checks if the value is an empty string, if not calls ism-func:cveSortOrderJoin if
                empty returns empty string.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
        <xd:param name="cveToSortWith"/>
    </xd:doc>
    <xsl:function name="ism-func:cveSortOrderJoinWithEmptyCheck">
        <xsl:param name="values"/>
        <xsl:param name="cveToSortWith" as="node()*"/>
        <xsl:choose>
            <xsl:when test="normalize-space($values) = ''">
                <xsl:value-of select="normalize-space($values)"/>
            </xsl:when>
            <xsl:otherwise>
                <xsl:variable name="tokens" select="xs:NMTOKENS(normalize-space($values))"
                    as="xs:NMTOKEN*"/>
                <xsl:value-of select="ism-func:cveSortOrderJoin($tokens, $cveToSortWith)"/>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Sorts multiple values according to a CVE passed in and returns the sorted set,
                removing duplicates.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
        <xd:param name="cveToSortWith"/>
    </xd:doc>
    <xsl:function name="ism-func:cveSortOrderJoin">
        <xsl:param name="values" as="xs:anyAtomicType*"/>
        <xsl:param name="cveToSortWith" as="node()*"/>
        <xsl:variable name="SortedValues" as="xs:NMTOKEN*">
            <xsl:perform-sort select="$values">
                <xsl:sort select="ism-func:cveSortOrder(., $cveToSortWith)" data-type="number"/>
            </xsl:perform-sort>
        </xsl:variable>
        <xsl:value-of select="ism-func:join(distinct-values($SortedValues))"/>
    </xsl:function>

    <xd:doc>
        <xd:desc> Returns the given sequence of $values joined into a normalized single string </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:join" as="xs:string">
        <xsl:param name="values" as="xs:anyAtomicType*"/>

        <xsl:sequence select="normalize-space(string-join($values, ' '))"/>
    </xsl:function>

    <xd:doc>
        <xd:desc> Return a list of values as a space delimited string from a sequence of tokens that
            only matches the regex </xd:desc>
        <xd:param name="attrValues"/>
        <xd:param name="regex"/>
    </xd:doc>
    <xsl:function name="ism-func:getStringFromSequenceWithOnlyRegexValues" as="xs:string">
        <xsl:param name="attrValues"/>
        <xsl:param name="regex"/>
        <xsl:variable name="StringWithOnlyRegexValues">
            <xsl:for-each select="$attrValues">
                <!-- if value does match the regex, return that value -->
                <xsl:if test="matches(current(), $regex)">
                    <xsl:value-of select="current()"/>
                </xsl:if>
                <xsl:value-of select="' '"/>
            </xsl:for-each>
        </xsl:variable>
        <xsl:value-of select="normalize-space(string($StringWithOnlyRegexValues))"/>
    </xsl:function>

    <xd:doc>
        <xd:desc> Return a list of values as a space delimited string from a sequence of tokens that
            filters out anything matching the regex </xd:desc>
        <xd:param name="attrValues"/>
        <xd:param name="regex"/>
    </xd:doc>
    <xsl:function name="ism-func:getStringFromSequenceWithoutRegexValues" as="xs:string">
        <xsl:param name="attrValues" as="xs:string*"/>
        <xsl:param name="regex"/>
        <xsl:variable name="StringWithoutRegexValues">
            <xsl:for-each select="$attrValues">
                <!-- if value does not match the regex, return that value -->
                <xsl:if test="not(matches(current(), $regex))">
                    <xsl:value-of select="current()"/>
                </xsl:if>
                <xsl:value-of select="' '"/>
            </xsl:for-each>
        </xsl:variable>
        <xsl:value-of select="normalize-space(string($StringWithoutRegexValues))"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>for running xspec tests and other debug</xd:desc>
        <xd:param name="attrValues"/>
    </xd:doc>
    <xsl:function name="ism-func:cveSortOrderDebug" as="xs:string">
        <xsl:param name="attrValues"/>
        <xsl:variable name="Sorted">
            <xsl:for-each select="$attrValues">
                <xsl:value-of select="current()"/>
                <xsl:text>:</xsl:text>
                <xsl:value-of select="xs:integer(ism-func:cveSortOrder(., $RelCVE))"/>
                <xsl:text>|</xsl:text>
            </xsl:for-each>
        </xsl:variable>
        <xsl:value-of select="$Sorted"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Sorts values from an ism:ownerProducer attribute to the right order for
                banner/portion rendering.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:sortOwnerProducer">
        <xsl:param name="values"/>
        <xsl:value-of
            select="ism-func:sortCountryAndTetraWithEmptyCheck($values, $OwnerProducerCVE)"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Sorts values from an ism:sciControls attribute to the right order for
                banner/portion rendering.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:sortSciControls">
        <xsl:param name="values"/>
        <xsl:value-of select="ism-func:sortJoin($values)"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Sorts values from an ism:sar attribute to the right order for banner/portion
                rendering.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:sortSar">
        <xsl:param name="values"/>
        <xsl:value-of select="ism-func:sortJoin($values)"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Sorts values from an ism:atomicenergymarkings attribute to the right order for
                banner/portion rendering.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:sortAtomicenergymarkings">
        <xsl:param name="values"/>
        <xsl:value-of select="ism-func:cveSortOrderJoinWithEmptyCheck($values, $AtomicEnergyCVE)"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Sorts values from an ism:fgiopen attribute to the right order for banner/portion
                rendering.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:sortFGIOpen">
        <xsl:param name="values"/>
        <xsl:value-of select="ism-func:sortCountryAndTetraWithEmptyCheck($values, $FGIOpenCVE)"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Sorts values from an ism:fgiProtected attribute to the right order for
                banner/portion rendering.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:sortFGIProtected">
        <xsl:param name="values"/>
        <xsl:value-of select="ism-func:sortCountryAndTetraWithEmptyCheck($values, $FGIProtectedCVE)"
        />
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Sorts values from an ism:dissemControls attribute to the right order for
                banner/portion rendering.</xd:p>
            <xd:p>Only used by Rollup 2021-02-23 When Rollup embraces CUI should go away.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:sortDissemControlsPreCUI">
        <xsl:param name="values"/>
        <xsl:value-of
            select="ism-func:cveSortOrderJoinWithEmptyCheck($values, $disseminationControlsCVE)"/>
    </xsl:function>
    
    <xd:doc>
        <xd:desc>
            <xd:p>Determines the right set of allowed ism:dissemControls values based on
                ism:compliesWith. Sorts values from the ism:dissemControls attribute to the right
                order for banner/portion rendering.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
        <xd:param name="compliesWith"/>
        <xd:param name="CUIandICcontrolMarkings"/>
    </xd:doc>
    <xsl:function name="ism-func:sortDissemControls">
        <xsl:param name="values"/>
        <xsl:param name="compliesWith"/>
        <xsl:param name="CUIandICcontrolMarkings"/>
        <xsl:variable name="disseminationControlsCVE">
            <xsl:choose>
                <xsl:when test="$compliesWith = 'USA-CUI-ONLY'">
                    <xsl:copy-of select="$disseminationControlsCUICVE"/>
                </xsl:when>
                <xsl:when test="contains($compliesWith, 'USA-CUI')">
                    <xsl:choose>
                        <xsl:when test="$CUIandICcontrolMarkings = false()">
                            <xsl:copy-of select="$disseminationControlsCUICVE"/>
                        </xsl:when>
                        <xsl:otherwise>
                            <xsl:copy-of select="$disseminationControlsCommingledCVE"/>
                        </xsl:otherwise>
                    </xsl:choose>
                </xsl:when>
                <xsl:otherwise>
                    <xsl:copy-of select="$disseminationControlsIcrmCVE"/>
                </xsl:otherwise>
            </xsl:choose>
        </xsl:variable>
        <xsl:value-of
            select="ism-func:cveSortOrderJoinWithEmptyCheck($values, $disseminationControlsCVE)"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>Determine the set of dissemination controls that are NOT CUI limited dissem
            controls. This variable is used to determine whether to use UNCLASSIFIDED or CUI in the
            banner or portion mark of an UNCLASSIFIED commingled document. If any of the
            dissemination controls is NOT one of the CUI limited dissem controls (i.e., it is a
            dissem control from the IC Markings Register and Manual that is NOT also one of the CUI
            limited dissem controls), then use UNCLASSIFIED rather than CUI at the start of the
            banner or portion mark.</xd:desc>
        <xd:param name="values"/>
    </xd:doc>
 <!--   <xsl:function name="ism-func:get.dissemNotCUI">
        <xsl:param name="values"/>
        <xsl:variable name="CUIdissems">
            <xsl:for-each select="$disseminationControlsCUICVE/cve:Term/cve:Value">
                <xsl:choose>
                    <xsl:when test="current() = 'NOFORN'">
                        <xsl:value-of select="'NF'"/>
                    </xsl:when>
                    <xsl:otherwise>
                        <xsl:value-of select="current()"/>
                    </xsl:otherwise>
                </xsl:choose>
                <xsl:if test="position() != last()">
                    <xsl:text> </xsl:text>
                </xsl:if>
            </xsl:for-each>
        </xsl:variable>
        <xsl:variable name="dissemNotCui">
            <xsl:if test="$values != ''">
                <xsl:for-each select="tokenize($values, ' ')">
                    <xsl:if test="not(contains($CUIdissems, current()))">
                        <xsl:value-of select="current()"/>
                    </xsl:if>
                </xsl:for-each>
            </xsl:if>
        </xsl:variable>
        <xsl:value-of select="$dissemNotCui"/>
    </xsl:function> -->
    
    <xsl:function name="ism-func:get.dissemNotCUI">
        <xsl:param name="values"/>
        <xsl:variable name="tokenizedDissems" select="tokenize($values, ' ')"/>
        <xsl:variable name="dissemNotCui">
            <xsl:for-each select="$tokenizedDissems">
                <xsl:if test="not($disseminationControlsCUICVE/cve:Term[cve:Value = current()])">
                    <xsl:if test="position() != 1">
                        <xsl:text> </xsl:text>
                    </xsl:if>
                    <xsl:value-of select="current()"/>
                </xsl:if>
            </xsl:for-each>
        </xsl:variable>
        <xsl:value-of select="normalize-space($dissemNotCui)"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>Determine the set of dissemination controls that ARE CUI limited dissem controls.
            This variable is used to generate a CUI dissems line in a CUI Control Block for DoD
            uses. </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:get.dissemCUI">
        <xsl:param name="values"/>
        <xsl:variable name="tokenizedDissems" select="tokenize($values, ' ')"/>
        <xsl:variable name="dissemCui">
            <xsl:for-each select="$tokenizedDissems">
                <xsl:if test="$disseminationControlsCUICVE/cve:Term[cve:Value = current()]">
                    <xsl:if test="position() != 1">
                        <xsl:text> </xsl:text>
                    </xsl:if>
                    <xsl:value-of select="current()"/>
                </xsl:if>
            </xsl:for-each>
        </xsl:variable>
        <xsl:value-of select="normalize-space($dissemCui)"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Sorts values from an ism:releaseto attribute to the right order for banner/portion
                rendering.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:sortReleaseto">
        <xsl:param name="values"/>
        <xsl:value-of select="ism-func:sortCountryAndTetraWithEmptyCheck($values, $RelCVE)"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Sorts values from an ism:displayonly attribute to the right order for
                banner/portion rendering.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:sortDisplayonly">
        <xsl:param name="values"/>
        <xsl:value-of select="ism-func:sortCountryAndTetraWithEmptyCheck($values, $RelCVE)"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Sorts values from an ism:nonic attribute to the right order for banner/portion
                rendering.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:sortNonic">
        <xsl:param name="values"/>
        <xsl:value-of select="ism-func:sortNonICWithEmptyCheck($values)"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Sorts values from an ism:cuiBasic attribute alphabetically for banner/portion
                rendering.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:sortCuiBasic">
        <xsl:param name="values"/>
        <xsl:value-of select="ism-func:sortJoin($values)"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Sorts values from an ism:cuiSpecified attribute alphabetically for banner/portion
                rendering.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:sortCuiSpecified">
        <xsl:param name="values"/>
        <xsl:value-of select="ism-func:sortJoin($values)"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Sorts values from an ism:secondBannerLine attribute alphabetically for
                banner/portion rendering.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:sortSecondBannerLine">
        <xsl:param name="values"/>
        <xsl:value-of select="ism-func:sortJoin($values)"/>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Checks if the value is an empty string, if not calls ism-func:sortCountryAndTetra
                if empty returns empty string.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
        <xd:param name="cveToSortWith"/>
    </xd:doc>
    <xsl:function name="ism-func:sortCountryAndTetraWithEmptyCheck">
        <xsl:param name="values"/>
        <xsl:param name="cveToSortWith" as="node()*"/>
        <xsl:choose>
            <xsl:when test="normalize-space($values) = ''">
                <xsl:value-of select="normalize-space($values)"/>
            </xsl:when>
            <xsl:otherwise>
                <xsl:variable name="tokens" select="xs:NMTOKENS(normalize-space($values))"
                    as="xs:NMTOKEN*"/>
                <xsl:value-of select="ism-func:sortCountryAndTetra($tokens, $cveToSortWith)"/>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:function>

    <xd:doc>
        <xd:desc> CVEnumISMCATFGIOpen, CVEnumISMCATFGIProtected, CVEnumISMCATOwnerProducer, and
            CVEnumISMCATRelTo need special sorting to account for NATO Nacs. All tokens up to and
            including NATO go according to the CVE order All NATO:xxx go alphabetical. After any
            NATO:xxx the remainder go in CVE order. </xd:desc>
        <xd:param name="attrValues"/>
        <xd:param name="cveToSortWith">
            <xd:p>The contents of the CVE that should be used for sorting.</xd:p>
        </xd:param>
    </xd:doc>
    <xsl:function name="ism-func:sortCountryAndTetra" as="xs:string">
        <xsl:param name="attrValues"/>
        <xsl:param name="cveToSortWith"/>
        <xsl:variable name="regex" select="'NATO:'"/>
        <xsl:variable name="sortedTokens" as="xs:string*">
            <xsl:variable name="beforeDistinct" as="xs:string*">
                <xsl:perform-sort select="$attrValues">
                    <xsl:sort select="xs:integer(ism-func:cveSortOrder(., $cveToSortWith))"
                        data-type="number"/>
                </xsl:perform-sort>
            </xsl:variable>
            <xsl:sequence select="distinct-values($beforeDistinct)"/>
        </xsl:variable>
        <xsl:variable name="withoutRegexValues" as="xs:NMTOKEN*">
            <xsl:variable name="StringWithout"
                select="ism-func:getStringFromSequenceWithoutRegexValues($sortedTokens, $regex)"/>
            <xsl:choose>
                <xsl:when test="normalize-space($StringWithout) != ''">
                    <xsl:sequence select="xs:NMTOKENS(normalize-space($StringWithout))"/>
                </xsl:when>
                <xsl:otherwise/>
            </xsl:choose>
        </xsl:variable>
        <xsl:choose>
            <xsl:when test="count($sortedTokens) = count($withoutRegexValues)">
                <xsl:value-of select="ism-func:join($sortedTokens)"/>
            </xsl:when>
            <xsl:otherwise>
                <xsl:variable name="nacSortOrder"
                    select="ism-func:cveSortOrder('NATO', $cveToSortWith)"/>
                <xsl:variable name="NacValues" as="xs:NMTOKEN*">
                    <xsl:perform-sort
                        select="xs:NMTOKENS(ism-func:getStringFromSequenceWithOnlyRegexValues($attrValues, $regex))">
                        <xsl:sort select="." order="ascending"/>
                    </xsl:perform-sort>
                </xsl:variable>
                <xsl:variable name="SortedValuesWithoutNac" as="xs:NMTOKEN*">
                    <xsl:perform-sort select="$withoutRegexValues">
                        <xsl:sort select="ism-func:cveSortOrder(., $cveToSortWith)"
                            data-type="number"/>
                    </xsl:perform-sort>
                </xsl:variable>
                <xsl:variable name="beforeNAC"
                    select="$SortedValuesWithoutNac[ism-func:cveSortOrder(., $cveToSortWith) &lt; $nacSortOrder]"
                    as="xs:NMTOKEN*"/>
                <xsl:variable name="afterNAC"
                    select="$SortedValuesWithoutNac[ism-func:cveSortOrder(., $cveToSortWith) &gt; $nacSortOrder]"
                    as="xs:NMTOKEN*"/>
                <xsl:value-of select="($beforeNAC, $NacValues, $afterNAC)"/>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Checks if the value is an empty string, if not calls ism-func:sortNonIC if empty
                returns empty string.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:sortNonICWithEmptyCheck">
        <xsl:param name="values"/>
        <xsl:choose>
            <xsl:when test="normalize-space($values) = ''">
                <xsl:value-of select="normalize-space($values)"/>
            </xsl:when>
            <xsl:otherwise>
                <xsl:variable name="tokens" select="xs:NMTOKENS(normalize-space($values))"
                    as="xs:NMTOKEN*"/>
                <xsl:value-of select="ism-func:sortNonIC($tokens)"/>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:function>

    <xd:doc>
        <xd:desc> CVEnumISMNonIC needs special sorting to account for ACCMs. The order is DS if
            present followed by all the ACCM-xxx tokens in alphabetical order followed by the
            remaining CVEnumISMNonIC values. </xd:desc>
        <xd:param name="attrValues"/>
    </xd:doc>
    <xsl:function name="ism-func:sortNonIC" as="xs:string">
        <xsl:param name="attrValues"/>
        <xsl:variable name="regex" select="'ACCM-'"/>
        <xsl:variable name="sortedTokens" as="xs:string*">
            <xsl:variable name="beforeDistinct" as="xs:string*">
                <xsl:perform-sort select="$attrValues">
                    <xsl:sort select="xs:integer(ism-func:cveSortOrder(., $NonICControlsCVE))"
                        data-type="number"/>
                </xsl:perform-sort>
            </xsl:variable>
            <xsl:sequence select="distinct-values($beforeDistinct)"/>
        </xsl:variable>
        <xsl:variable name="withoutRegexValues" as="xs:NMTOKEN*">
            <xsl:variable name="StringWithout"
                select="ism-func:getStringFromSequenceWithoutRegexValues($sortedTokens, $regex)"/>
            <xsl:choose>
                <xsl:when test="normalize-space($StringWithout) != ''">
                    <xsl:sequence select="xs:NMTOKENS(normalize-space($StringWithout))"/>
                </xsl:when>
                <xsl:otherwise/>
            </xsl:choose>
        </xsl:variable>
        <xsl:choose>
            <xsl:when test="count($sortedTokens) = count($withoutRegexValues)">
                <xsl:value-of select="ism-func:join($sortedTokens)"/>
            </xsl:when>
            <xsl:otherwise>
                <xsl:variable name="ds-SortOrder"
                    select="ism-func:cveSortOrder('DS', $NonICControlsCVE)"/>
                <xsl:variable name="accmValues" as="xs:NMTOKEN*">
                    <xsl:perform-sort
                        select="xs:NMTOKENS(ism-func:getStringFromSequenceWithOnlyRegexValues($attrValues, $regex))">
                        <xsl:sort select="." order="ascending"/>
                    </xsl:perform-sort>
                </xsl:variable>
                <xsl:variable name="SortedValuesWithoutACCM" as="xs:NMTOKEN*">
                    <xsl:perform-sort select="$withoutRegexValues">
                        <xsl:sort select="ism-func:cveSortOrder(., $NonICControlsCVE)"
                            data-type="number"/>
                    </xsl:perform-sort>
                </xsl:variable>
                <xsl:variable name="beforeAccm"
                    select="$SortedValuesWithoutACCM[ism-func:cveSortOrder(., $NonICControlsCVE) &lt;= $ds-SortOrder]"
                    as="xs:NMTOKEN*"/>
                <xsl:variable name="afterACCM"
                    select="$SortedValuesWithoutACCM[ism-func:cveSortOrder(., $NonICControlsCVE) &gt; $ds-SortOrder]"
                    as="xs:NMTOKEN*"/>
                <xsl:value-of select="($beforeAccm, $accmValues, $afterACCM)"/>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Sorts values alphabetically and string joins with spaces.</xd:p>
        </xd:desc>
        <xd:param name="values"/>
    </xd:doc>
    <xsl:function name="ism-func:sortJoin">
        <xsl:param name="values"/>
        <xsl:variable name="tokens">
            <xsl:perform-sort select="tokenize(normalize-space($values), ' ')">
                <xsl:sort select="." order="ascending"/>
            </xsl:perform-sort>
        </xsl:variable>
        <xsl:value-of select="string-join($tokens, ' ')"/>
    </xsl:function>
    
    <xd:doc>
        <xd:desc> Recursively remove all decomposable tetragraphs in the given $relTo string 
            and replace them with their constituent countries. Note: Does not include USA </xd:desc>
        <xd:param name="relTo"/>
    </xd:doc>
    <xsl:function
        name="ism-func:expandDecomposableTetras"
        as="xs:string*">
        <xsl:param name="relTo" as="xs:string"/>
        
        <xsl:variable name="expandedTetras">
            <xsl:choose>
                <xsl:when test="ism-func:containsDecomposableTetra($relTo)">
                    <xsl:variable name="currTetra"
                        select="ism-func:tokenize($relTo)[. = $decomposableTetras][1]"/>
                    <xsl:variable name="currTetraCountries"
                        select="ism-func:join(ism-func:getCountriesForTetra($currTetra))"/>
                    <xsl:variable name="expandCurrTetra"
                        select="replace(ism-func:padValue($relTo), ism-func:padValue($currTetra), ism-func:padValue($currTetraCountries))"/>
                    
                    <xsl:value-of select="ism-func:expandDecomposableTetras($expandCurrTetra)"/>
                </xsl:when>
                
                <xsl:otherwise>
                    <xsl:value-of select="normalize-space($relTo)"/>
                </xsl:otherwise>
            </xsl:choose>
        </xsl:variable>
        
        <xsl:sequence select="distinct-values(ism-func:tokenize($expandedTetras))[. != 'USA']"/>
    </xsl:function>
    
    <xd:doc>
        <xd:desc> Returns true if the given $relTo string (e.g. 'USA CAN GBR') contains any 
            tetragraphs that can be decomposed into its constituent countries  </xd:desc>
        <xd:param name="relTo"/>
    </xd:doc>
    <xsl:function
        name="ism-func:containsDecomposableTetra"
        as="xs:boolean">
        <xsl:param name="relTo" as="xs:string?"/>
        
        <xsl:sequence select="normalize-space($relTo) and ism-func:containsAnyOfTheTokens($relTo, $decomposableTetras)"/>
    </xsl:function>
    
    <xd:doc>
        <xd:desc>
            Returns true if any token in the attribute value matches at least one token in the provided list.
        </xd:desc>
        <xd:param name="attribute"/>
        <xd:param name="tokenList"/>
    </xd:doc>
    <xsl:function
        name="ism-func:containsAnyOfTheTokens"
        as="xs:boolean">
        <xsl:param name="attribute"/>
        <xsl:param name="tokenList" as="xs:string*"/>
        <xsl:sequence select="some $attrToken in tokenize(normalize-space(string($attribute)), ' ') satisfies $attrToken = $tokenList"/>
    </xsl:function>
    
    <xd:doc>
        <xd:desc> Returns the sequence of country codes that correspond to the given $tetra </xd:desc>
        <xd:param name="tetra"/>
    </xd:doc>
    <xsl:function
        name="ism-func:getCountriesForTetra"
        as="xs:string*">
        <xsl:param name="tetra" as="xs:string"/>
        
        <xsl:sequence select="$decomposableTetraElems[catt:TetraToken/text() = $tetra]/catt:Membership/*/text()"/>
    </xsl:function>
    
    <xd:doc>
        <xd:desc> Returns normalized $value with a preceding and subsequent space (' ') character </xd:desc>
        <xd:param name="value"/>
    </xd:doc>
    <xsl:function
        name="ism-func:padValue"
        as="xs:string">
        <xsl:param name="value" as="xs:string?"/>
        
        <xsl:value-of select="concat(' ', normalize-space($value), ' ')"/>
    </xsl:function>
    
    <xd:doc>
        <xd:desc> Returns the given $value with its values broken into tokens using whitespace as delimiters </xd:desc>
        <xd:param name="value"/>
    </xd:doc>
    <xsl:function
        name="ism-func:tokenize"
        as="xs:string*">
        <xsl:param name="value" as="xs:string?"/>
        
        <xsl:sequence select="tokenize(normalize-space($value), ' ')"/>
    </xsl:function>

</xsl:stylesheet>
