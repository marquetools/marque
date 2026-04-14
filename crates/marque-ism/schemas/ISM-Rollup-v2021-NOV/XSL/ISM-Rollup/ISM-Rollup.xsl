<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:xs="http://www.w3.org/2001/XMLSchema"
    xmlns:xd="http://www.oxygenxml.com/ns/doc/xsl"
    xmlns:cve="urn:us:gov:ic:cve"  xmlns:catt="urn:us:gov:ic:taxonomy:catt:tetragraph"
    xmlns:util="urn:us:gov:ic:ism-rollup:xsl:util"
    xmlns:arh="urn:us:gov:ic:arh" 
    xmlns:functx="http://www.functx.com"
    xmlns:ism-func="urn:us:gov:ic:ism:functions"
    xmlns:banner="banner.fn" 
    xmlns:ism="urn:us:gov:ic:ism" 
    xmlns:ntk="urn:us:gov:ic:ntk" 
    exclude-result-prefixes="xs xd cve util catt banner ism-func functx" version="2.0">
    
    <xsl:import href="functx-1.0-doc-2007-01.xsl"/>
    <xsl:import href="../../XSL/ISM/IC-ISM-Functions.xsl"/>
    
    <xd:doc scope="stylesheet">
        <xd:desc>
            <xd:p><xd:b>Created on:</xd:b> Aug 11, 2018</xd:p>
            <xd:p><xd:b>Author:</xd:b>IC-CIO</xd:p>
            <xd:p>Implementation of security markings Rollup using XSLT 2.0. </xd:p>
            <xd:p>Depends on ISMCAT and ISM CVEs and ISMCAT Taxonomy. </xd:p>
            <xd:p>example invocations</xd:p>
            <xd:ul>
                <xd:li>
                    <xd:p>Invocation using dummy paths but showing all parameters </xd:p>
                    <xd:pre>java -jar PathTo/saxon9he.jar -xsl:PathTo/ISM-Rollup.xsl -o:PathTo/OutputRolledUp.xml -s:PathTo/SomeSource.xml \
                        derivedFrom='Audit Record Portion markings' \
                        derivativelyClassifiedBy='Bill Smith Govt Accreditor of X' \
                        pathToISMCATCVE='../../CVE/ISMCAT/' \
                        pathToISMCVE='../../CVE/ISM/' \
                        pathToISMCATTaxonomy='../../Taxonomy/ISMCAT/'
                    </xd:pre>
                </xd:li>
              
            </xd:ul>
            <xd:p>Utilizes functions from http://www.xsltfunctions.com an GNU Lesser General Public
                License as published by the Free Software Foundation; either version 2.1 of the License. The functions are in
            A separate file imported to this XSL. None are customized they are used as is.</xd:p>
        </xd:desc>
    </xd:doc>

    <xsl:param name="derivedFrom" select="'Portion markings of document sent to rollup Tool'"/>
    <xsl:param name="derivativelyClassifiedBy" select="'Human who accredited use of rollup tool in production'"/>
    <xsl:param name="pathToISMCATCVE" select="'../../CVE/ISMCAT/'"></xsl:param>
    <xsl:param name="pathToISMCVE" select="'../../CVE/ISM/'"></xsl:param>
    <!-- DY: migrated to IC-ISM-Functions.xsl -->
    <!--xsl:param name="pathToISMCATTaxonomy" select="'../../Taxonomy/ISMCAT/'"></xsl:param-->
    
    <xsl:output method="xml" indent="yes"/>

    <xsl:key name="classification" match="*[@ism:classification and util:contributesToRollup(.)]" use="@ism:classification"/>
    <xsl:variable name="FGIOpenCVE" select="document(concat($pathToISMCATCVE,'CVEnumISMCATFGIOpen.xml'))//cve:CVE/cve:Enumeration"></xsl:variable>
    <xsl:variable name="RelCVE" select="document(concat($pathToISMCATCVE,'CVEnumISMCATRelTo.xml'))//cve:CVE/cve:Enumeration"></xsl:variable>
    <xsl:variable name="disseminationControlsCVE" select="document(concat($pathToISMCVE,'CVEnumISMDissem.xml'))//cve:CVE/cve:Enumeration"></xsl:variable>
    
 <!-- lifted from ISM -->
    <xsl:variable name="partTags" select="util:partTags(/*)"/>
    
    <xsl:variable name="countFdrPortions" select="util:countFdrPortions(/*)"/>
    
    <!-- DY: migrated to IC-ISM-Functions.xsl -->
    <!--xsl:variable name="catt"
        select="document(concat($pathToISMCATTaxonomy,'TetragraphTaxonomyDenormalized.xml'))"/-->
    
    <!-- DY: migrated to IC-ISM-Functions.xsl -->
    <!--xsl:variable name="cattMappings" select="$catt//catt:Tetragraph"/-->
    
    <xsl:variable name="tetragraphList"
        select="document(concat($pathToISMCATCVE,'CVEnumISMCATTetragraph.xml'))//cve:CVE/cve:Enumeration/cve:Term/cve:Value"/>
    
    <!-- DY: migrated to IC-ISM-Functions.xsl -->
    <!--xsl:variable name="decomposableTetraElems"
        select="$cattMappings[@decomposable[. = 'Yes' or . = 'NA']]"/-->
    
    <!-- DY: migrated to IC-ISM-Functions.xsl -->
    <!--xsl:variable name="decomposableTetras"
        select="$decomposableTetraElems/catt:TetraToken/text()"/-->
    
    <!-- Either comment this line out or create a XSL that imports ISM-Rollup with only this line. It makes XSpec tests work
        Since XSpec does not handle global variables well but this not being global is SUPER slow.
        So you Would only want this during XSpec as it would return to the very slow execution. -->
    <!--    <xsl:variable name="ISM_RESOURCE_ELEMENT" select="/parent::*"/>
-->
    <xsl:variable name="resourceElement" select="util:resourceElementTraverse((root(.)))"/>
 
    <xd:doc>
        <xd:desc>
            <xd:p>Main template match from which all work starts. It is matching on the resource node for the document passed in since that is the only node
                that is actually modified. All other nodes are passed through with identity transform.</xd:p>
        </xd:desc>
    </xd:doc>
    <xsl:template match="*[generate-id(.) = generate-id(util:resourceElement((root(.))))]">
        <xsl:variable name="classifiedDoc" as="xs:boolean">
            <xsl:value-of select="util:classifiedDoc(/)"/>
        </xsl:variable>
        <xsl:variable name="resourceNodeContext" select="."/>
        <xsl:copy>
            <xsl:apply-templates select="@* except @ism:*"/>
            <xsl:apply-templates
                select="@ism:DESVersion | @ism:createDate | @ism:resourceElement | @ism:ownerProducer | @ism:compliesWith | @ism:ISMCATCESVersion"/>
            <xsl:call-template name="addISMderived">
                <xsl:with-param name="derivativelyClassifiedBy" select="$derivativelyClassifiedBy"/>
                <xsl:with-param name="derivedFrom" select="$derivedFrom"/>
            </xsl:call-template>
            <xsl:call-template name="addISMclassification"/>
            <xsl:call-template name="AddISMatomicEnergyMarkings"/>
            <xsl:call-template name="AddISMnonUSControls"/>
            <xsl:call-template name="AddISMFGI"/>
            
            <xsl:call-template name="AddISMSCIcontrols"/>
            <xsl:call-template name="AddISMhasApproximateMarkings"/>
            <xsl:call-template name="AddISMdisseminationControls">
                <xsl:with-param name="ResourceNodeContext" select="$resourceNodeContext"/>
            </xsl:call-template>
            <xsl:call-template name="AddISMdeclass">
                <xsl:with-param name="ResourceNodeContext" select="$resourceNodeContext"/>
            </xsl:call-template>
            <xsl:call-template name="AddNTK">
                <xsl:with-param name="ResourceNodeContext" select="$resourceNodeContext"/>
            </xsl:call-template>
            <xsl:call-template name="AddNotices">
                <xsl:with-param name="ResourceNodeContext" select="$resourceNodeContext"/>
            </xsl:call-template>
            <xsl:apply-templates select="node() except ntk:Access"/>
        </xsl:copy>
    </xsl:template>
    
    <xd:doc>
        <xd:desc>
            <xd:p>Create an ISM disseminationControls, and nonICmarkings attributes  based on values in the document.</xd:p>
            <xd:p>Handles the following  disseminationControls special cases</xd:p>
            <xd:ul>
                <xd:li>FOUO drops from classified Doc</xd:li>
                <xd:li>FOUO drops from DSEN Doc</xd:li>
                <xd:li>nonICmarkings SBU-NF adds NF in classified Doc</xd:li>
                <xd:li>nonICmarkings LES-NF adds NF in classified Doc</xd:li>
                <xd:li>NF anywhere drops REL, EYES, RELIDO, and DisplayOnly</xd:li>
            </xd:ul>
            <xd:p>Handles the following  nonICmarkings special cases</xd:p>
            <xd:ul>
                <xd:li>SBU-NF changes to SBU in a classified Doc or when NF is already present</xd:li>
                <xd:li>LES-NF changes to LES in a classified Doc or when NF is already present</xd:li>
            </xd:ul>
        </xd:desc>
        <xd:param name="ResourceNodeContext">
            <xd:p>The resource node for the document or fragment being rolled up. </xd:p>
        </xd:param>
    </xd:doc>
    <xsl:template name="AddISMdisseminationControls">
        <xsl:param name="ResourceNodeContext" required="yes"/>
        <xsl:variable name="distinctDisseminationControls"
            select="distinct-values(//*[util:contributesToRollup(.) and @ism:disseminationControls]/xs:NMTOKENS(@ism:disseminationControls))"/>
        <xsl:variable name="distinctnonICmarkings"
            select="distinct-values(//*[util:contributesToRollup(.) and @ism:nonICmarkings]/xs:NMTOKENS(@ism:nonICmarkings))"/>
        
        <xsl:variable name="allOCPortions" select="//*[util:contributesToRollup(.) and xs:NMTOKENS(@ism:disseminationControls)='OC']"/>
        <xsl:variable name="OCWithUSGov" select="$allOCPortions[some $dissem in xs:NMTOKENS(@ism:disseminationControls) satisfies $dissem ='OC-USGOV']"/>
        <xsl:variable name="OCWithOutUSGov" select="$allOCPortions[every $dissem in xs:NMTOKENS(@ism:disseminationControls) satisfies $dissem !='OC-USGOV']"/>
        <xsl:variable name="DropOCUSGOV" as="xs:boolean">
            <xsl:choose>
                <xsl:when test="count($OCWithUSGov) >0 and count($OCWithOutUSGov)>0">
                    <xsl:sequence select="true()"/>
                </xsl:when>
                <xsl:otherwise>
                    <xsl:sequence select="false()"/>
                </xsl:otherwise>
            </xsl:choose>
        </xsl:variable>
        
        <xsl:if test="count($distinctDisseminationControls) != 0 or count($distinctnonICmarkings) != 0">
            <xsl:variable name="droppedWithNF" select="xs:NMTOKENS('REL DISPLAYONLY EYES RELIDO')"/>
            <xsl:variable name="NF" select="xs:NMTOKENS('NF')"/>
            <xsl:variable name="allPortionsRelido" as="xs:boolean"
                select="util:allDissemPortionsHave($ResourceNodeContext, $distinctDisseminationControls, xs:NMTOKENS('RELIDO'))"/>
            
            <xsl:variable name="allPortionsRelEyes" as="xs:boolean"
                select="util:allDissemPortionsHave($ResourceNodeContext, $distinctDisseminationControls, xs:NMTOKENS('REL EYES'))"/>
            
            <xsl:variable name="allPortionsRelEyesorDisplay" as="xs:boolean"
                select="util:allDissemPortionsHave($ResourceNodeContext, $distinctDisseminationControls, xs:NMTOKENS('REL EYES DISPLAYONLY'))"/>
            
            <xsl:variable name="anyPortionHasDSEN" select="$distinctDisseminationControls = xs:NMTOKENS('DSEN')" as="xs:boolean"/>
            
            <!-- It might be NF and not meet this variable but it can't be REL,EYES, or DISPLAYONLY if it meets these criteria -->
            <xsl:variable name="CertainNFDocument" as="xs:boolean">
                <xsl:choose>
                    <xsl:when test="$distinctDisseminationControls = $NF">
                        <xsl:copy-of select="true()"/>
                    </xsl:when>
                    <xsl:when test="not($allPortionsRelEyesorDisplay) and $distinctDisseminationControls =xs:NMTOKENS('REL EYES DISPLAYONLY')">
                        <xsl:copy-of select="true()"/>
                    </xsl:when>
                    <xsl:when test="$distinctnonICmarkings = xs:NMTOKENS('LES-NF SBU-NF')">
                        <xsl:copy-of select="true()"/>
                    </xsl:when>
                    <xsl:otherwise>
                        <xsl:copy-of select="false()"/>
                    </xsl:otherwise>
                </xsl:choose>
                
            </xsl:variable>

            <xsl:variable name="dissemFiltered1" as="xs:NMTOKEN*">
                <xsl:for-each select="$distinctDisseminationControls">
                    <xsl:choose>
                        <xsl:when test="not($allPortionsRelido) and (. = 'RELIDO')"/>
                        <xsl:when test="$DropOCUSGOV and . = 'OC-USGOV' "/>
                        <xsl:when test="$CertainNFDocument and (. = 'REL' or . = 'DISPLAYONLY' or . = 'EYES' or . = 'RELIDO')"/>
                        <xsl:when test="not($allPortionsRelEyes) and (. = 'REL' or . = 'EYES')"/>
                        <xsl:when test="not($allPortionsRelEyesorDisplay) and (. = 'DISPLAYONLY')"/>
                        <xsl:when test="$anyPortionHasDSEN and (. = 'FOUO')"/>
                        <xsl:when test="util:classifiedDoc(root($ResourceNodeContext)) and not($anyPortionHasDSEN) and (. = 'FOUO')"/>
                        
                        <xsl:otherwise>
                            <xsl:copy-of select="xs:NMTOKEN(current())"/>
                        </xsl:otherwise>
                    </xsl:choose>
                </xsl:for-each>
                <xsl:if test="$distinctDisseminationControls != $NF and $CertainNFDocument">
                    <xsl:copy-of select="xs:NMTOKEN('NF')"/>
                </xsl:if>
            </xsl:variable>

            <xsl:variable name="nonICmarkingsFiltered1" as="xs:NMTOKEN*">
                <xsl:for-each select="$distinctnonICmarkings">
                    <xsl:choose>
                        <xsl:when test="($distinctDisseminationControls = ('NF', 'REL', 'DISPLAYONLY') or util:classifiedDoc(root($ResourceNodeContext))) and (. = 'SBU-NF')">
                            <xsl:copy-of select="xs:NMTOKEN('SBU')"/>
                        </xsl:when>
                        <xsl:when test="($distinctDisseminationControls = ('NF', 'REL', 'DISPLAYONLY') or  util:classifiedDoc(root($ResourceNodeContext))) and (. = 'LES-NF')">
                            <xsl:copy-of select="xs:NMTOKEN('LES')"/>
                        </xsl:when>
                        <xsl:otherwise>
                            <xsl:copy-of select="xs:NMTOKEN(current())"/>
                        </xsl:otherwise>
                    </xsl:choose>
                </xsl:for-each>
            </xsl:variable>
            <xsl:if test="count($nonICmarkingsFiltered1) !=0">
                <xsl:attribute name="nonICmarkings" namespace="urn:us:gov:ic:ism">
                    <xsl:value-of select="ism-func:join($nonICmarkingsFiltered1)"/>
                </xsl:attribute>
            </xsl:if>
            
            <xsl:variable name="DistinctRelPortions" select="distinct-values(//*[util:contributesToRollup(.) and @ism:releasableTo]/@ism:releasableTo)"/>
            <xsl:variable name="RelCommonCountriesUnsorted" select="util:unsortedCommonRelTokens($DistinctRelPortions,count($DistinctRelPortions))"/>
              
            <xsl:if test="count($dissemFiltered1) != 0">
                <xsl:variable name="dissemFiltered2" as="xs:NMTOKEN*">
                    <xsl:choose>
                        <xsl:when test="$dissemFiltered1 = xs:NMTOKENS('REL EYES')">
                            <xsl:variable name="distinctReleasableTo"
                                select="distinct-values(//*[util:contributesToRollup(.) and @ism:releasableTo]/xs:NMTOKENS(@ism:releasableTo))"/>
                            <xsl:choose>
                                <!-- There are common countries USA and something and at least on portion is REL drop EYES -->
                                <xsl:when test="count($RelCommonCountriesUnsorted) &gt; 1 and $dissemFiltered1 = xs:NMTOKENS('REL')">
                                    <xsl:copy-of select="$dissemFiltered1[not(. = xs:NMTOKENS('EYES'))]"/>
                                </xsl:when>
                                <!-- There are common countries USA and something and at no portion is REL keep EYES -->
                                <xsl:when test="count($RelCommonCountriesUnsorted) &gt; 1 and $dissemFiltered1 != xs:NMTOKENS('REL')">
                                    <xsl:copy-of select="$dissemFiltered1"/>
                                </xsl:when>
                                <xsl:otherwise>
                                    <xsl:copy-of select="$dissemFiltered1[not(. = xs:NMTOKENS('REL'))]"/>
                                    <xsl:copy-of select="xs:NMTOKENS('NF')"/>
                                </xsl:otherwise>
                            </xsl:choose>
                        </xsl:when>
                        <xsl:otherwise>
                            <xsl:copy-of select="$dissemFiltered1"/>
                        </xsl:otherwise>
                    </xsl:choose>
                </xsl:variable>
               
                <xsl:attribute name="disseminationControls" namespace="urn:us:gov:ic:ism">
                    <xsl:value-of select="ism-func:sortDissemControlsPreCUI(ism-func:join($dissemFiltered2))"/>
                </xsl:attribute>
                
                <xsl:if test="$dissemFiltered2 = xs:NMTOKENS('REL EYES')">
                    <xsl:attribute name="releasableTo" namespace="urn:us:gov:ic:ism">
                         <xsl:value-of select="ism-func:sortReleaseto(ism-func:join($RelCommonCountriesUnsorted))"/> 
                    </xsl:attribute>
                </xsl:if>
            </xsl:if>
        </xsl:if>
    </xsl:template>

    <xd:doc>
        <xd:desc>
            <xd:p>Create an ISM hasApproximateMarkings attribute based on values in the document.</xd:p>
            <xd:p>There are no known special cases to handle at this time.</xd:p>

        </xd:desc>
    </xd:doc>
    <xsl:template name="AddISMhasApproximateMarkings">
        <xsl:variable name="distincthasApproximateMarkings"
            select="distinct-values(//*[util:contributesToRollup(.) and @ism:hasApproximateMarkings]/@ism:hasApproximateMarkings)"/>
        <xsl:if test="(count($distincthasApproximateMarkings) != 0) and $distincthasApproximateMarkings = ('true', '1')">
            <xsl:attribute name="hasApproximateMarkings" namespace="urn:us:gov:ic:ism">
                <xsl:value-of select="true()"/>
            </xsl:attribute>
        </xsl:if>
    </xsl:template>
    
    <xd:doc>
        <xd:desc>
            <xd:p>Create an ISM ism:NoticeList structure based on values in the document. 
                Assumes using ARH and that resource node was the arh:security element structure.
            </xd:p>
        </xd:desc>
        <xd:param name="ResourceNodeContext">
            <xd:p>The resource node for the document or fragment being rolled up. </xd:p>
        </xd:param>
    </xd:doc>
    <xsl:template name="AddNotices">
        <xsl:param name="ResourceNodeContext" required="yes"/>
        
        <xsl:if test="//ism:NoticeList and $ResourceNodeContext/self::arh:Security">
            <xsl:variable name="tempNoticeList">
                <ism:NoticeList ism:classification="U" ism:resourceElement="true"
                    ism:ownerProducer="USA">
                    <!-- Notices have no variablilty so there is no need to normalize before distinct-deep compare. -->
                    <xsl:variable name="distinctNotices" select="functx:distinct-deep(//ism:Notice)"/>
                    <xsl:copy-of select="$distinctNotices"/>
                </ism:NoticeList>
            </xsl:variable>
            <xsl:variable name="RollupDoneNoticeList">
                <xsl:apply-templates select="$tempNoticeList"/>
            </xsl:variable>
            <xsl:apply-templates select="$RollupDoneNoticeList" mode="stripBlockAttributes"/>
        </xsl:if>

    </xsl:template>
    
    <xd:doc>
        <xd:desc>
            <xd:p>Create an ISM ntk:Access structure based on values in the document. 
                Assumes using ARH and that resource node was the arh:security element structure.
            </xd:p>
        </xd:desc>
        <xd:param name="ResourceNodeContext">
            <xd:p>The resource node for the document or fragment being rolled up. </xd:p>
        </xd:param>
    </xd:doc>
    <xsl:template name="AddNTK">
        <xsl:param name="ResourceNodeContext" required="yes"/>
        
        <xsl:if test="//ntk:Access and $ResourceNodeContext/self::arh:Security">
            <xsl:variable name="tempNTKAccess">
                <ntk:Access xmlns:ntk="urn:us:gov:ic:ntk"
                    ism:classification="U"
                    ism:ownerProducer="USA"
                    ism:resourceElement='true'>
                    <xsl:choose>
                        <xsl:when test="count(//ntk:Access)=1">
                            <!-- Only 1 NTK in the doc nothing to rollup just copy -->
                            <xsl:copy-of select="//ntk:Access/ntk:*"/>
                        </xsl:when>
                        <xsl:otherwise>
                            <xsl:if test="//ntk:RequiresAllOf">
                                <xsl:call-template name="processNTKAllOf"/>
                            </xsl:if>
                            <xsl:if test="//ntk:RequiresAnyOf">
                                <xsl:call-template name="processNTKAnyOf"/>
                            </xsl:if>
                        </xsl:otherwise>
                    </xsl:choose>                           
                </ntk:Access>
            </xsl:variable>

            <xsl:variable name="RollupDonetempNTKAccess">
                <xsl:apply-templates select="$tempNTKAccess"/>
            </xsl:variable>
            <xsl:apply-templates select="$RollupDonetempNTKAccess" mode="stripBlockAttributes"/>
        </xsl:if>
    </xsl:template>
    
    <xsl:template name="processNTKAllOf">
        <xsl:if test="count(//ntk:Access//ntk:RequiresAllOf/*[not(self::ntk:AccessProfileList)])>1 ">
            <!-- OLD probably 2013 NTK  we can't cope.-->
            <ntk:NTK-13-unsupported>There were NTK structures that pre 2015 and rollup was not defined to handle them.</ntk:NTK-13-unsupported>
        </xsl:if>
        <ntk:RequiresAllOf>
            <ntk:AccessProfileList>
                <!-- NTK need to normalize before distinct-deep compare. -->
                <xsl:variable name="normalizedNTKProfiles">
                    <xsl:apply-templates mode="normalizeNTKProfile" select="//ntk:RequiresAllOf/ntk:AccessProfileList/ntk:AccessProfile"/>
                </xsl:variable>
                <xsl:variable name="distinctNTKProfiles" select="functx:distinct-deep($normalizedNTKProfiles/ntk:AccessProfile)"/>
                
                <xsl:copy-of select="$distinctNTKProfiles"/>
              
            </ntk:AccessProfileList>
        </ntk:RequiresAllOf>
    </xsl:template>

    <xsl:template name="processNTKAnyOf">
        <xsl:variable name="normalizedNTKAnyOfProfiles">
            <xsl:apply-templates mode="normalizeNTKProfile" select="//ntk:RequiresAnyOf/ntk:AccessProfileList/ntk:AccessProfile"/>
        </xsl:variable>
        <xsl:variable name="distinctNTKAnyOfProfiles" select="functx:distinct-deep($normalizedNTKAnyOfProfiles/ntk:AccessProfile)"/>
        
        <xsl:variable name="normalizedNTKAnyOfAccessGroup">
            <xsl:apply-templates mode="normalizeNTKProfile" select="//ntk:RequiresAnyOf/ntk:AccessGroupList/ntk:AccessGroup"/>
        </xsl:variable>
         <xsl:variable name="distinctNTKAnyOfAccessGroup" select="functx:distinct-deep($normalizedNTKAnyOfAccessGroup/ntk:AccessGroup)"/>
        
        <xsl:choose>
            <xsl:when test="count(//ntk:Access//ntk:RequiresAnyOf)>1 and count($distinctNTKAnyOfProfiles)=1 and count($distinctNTKAnyOfAccessGroup)=0">
                <!-- There is exactly 1 anyOf profile use it and be happy. There may be some whose ntk profile has A or A a somewhat pointless anyOf but we don't
                            care. There also was not any NTK 2013 group element to worry. -->
                <ntk:RequiresAnyOf>
                    <ntk:AccessProfileList>
                        <xsl:copy-of select="($distinctNTKAnyOfProfiles)"/>
                    </ntk:AccessProfileList>
                </ntk:RequiresAnyOf>
            </xsl:when>
            <xsl:when test="count(//ntk:Access//ntk:RequiresAnyOf)>1 and count($distinctNTKAnyOfProfiles)=0 and count($distinctNTKAnyOfAccessGroup)=1">
                <!-- There is exactly 1 anyOf AccessGroup use it and be happy even if it is from NTK 2013. There may be some whose ntk AccessGroup has A or A a somewhat pointless anyOf but we don't
                            care. There also was not any NTK profile element to worry. -->
                <!-- NOTE that since this appears to be NTK from 2013 or earlier the ntk:RequiresAnyOf had ISM atrributes on it. ISM 2015 and later do not.  -->
                <ntk:RequiresAnyOf ism:classification="U" ism:ownerProducer="USA">
                    <ntk:AccessGroupList>
                        <xsl:copy-of select="($distinctNTKAnyOfAccessGroup)"/>
                    </ntk:AccessGroupList>
                </ntk:RequiresAnyOf>
            </xsl:when>
            <xsl:when test="count(//ntk:Access//ntk:RequiresAnyOf)>1 and ( count($distinctNTKAnyOfProfiles)>1 or count($distinctNTKAnyOfAccessGroup)>1)">
                <!-- More than 1 ntk:RequiresAnyOf we can't do rollup. Cause a bad NTK structure to fail validation. This is an intentional invalid marking to cause validation to fail on bad input.-->
                <ntk:TooManyRequiresAnyOf>There were more than 1 RequiresAnyOf rollup not possible</ntk:TooManyRequiresAnyOf>
            </xsl:when>
            <xsl:when test="count(//ntk:Access//ntk:RequiresAnyOf/*[not(self::ntk:AccessProfileList)])>1 ">
                <!-- OLD probably 2013 NTK with multiple anyOF we can't cope.-->
                <ntk:NTK-13-unsupported>There were more than 1 RequiresAnyOf and they were pre 2015 so rollup not possible</ntk:NTK-13-unsupported>
            </xsl:when>
            
            <xsl:otherwise>
                <!-- Copy the 1 distinct ntk:RequiresAnyOf if it exists -->
                <xsl:copy-of select="(//ntk:Access//ntk:RequiresAnyOf)[1]"/>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:template>

    <xd:doc>
        <xd:desc>
            <xd:p>Create an ISM declassDate attribute based on values in the document.</xd:p>
            <xd:p>Handles the following special cases</xd:p>
            <xd:ul>
                <xd:li>If there are RD,FRD, or TFNI AEA markings add the AEA exception.</xd:li>
                <xd:li>If there are any NATO/NATO:NAC portion or NATO/NATO:NAC FGI add the NATO exception.</xd:li>
            </xd:ul>
        </xd:desc>
        <xd:param name="ResourceNodeContext">
            <xd:p>The resource node for the document or fragment being rolled up. </xd:p>
        </xd:param>
    </xd:doc>
    <xsl:template name="AddISMdeclass">
        <xsl:param name="ResourceNodeContext" required="yes"/>
        <xsl:if test="util:classifiedDoc(root($ResourceNodeContext))">
            <xsl:variable name="distinctdeclassDate" select="distinct-values(//*[util:contributesToRollup(.) and @ism:declassDate]/@ism:declassDate)"
                as="xs:date*"/>
            <xsl:variable name="distinctdeclassEvent" select="distinct-values(//*[util:contributesToRollup(.) and @ism:declassEvent]/@ism:declassEvent)"/>
            <xsl:variable name="distinctdeclassException"
                select="distinct-values(//*[util:contributesToRollup(.) and @ism:declassException]/xs:NMTOKENS(@ism:declassException))"/>

            <xsl:variable name="countNATO" as="xs:integer"
                select="count(//*[util:contributesToRollup(.) and (starts-with(@ism:ownerProducer, 'NATO') or contains(@ism:FGIsourceOpen, 'NATO'))])"/>
            <xsl:variable name="countAEA" as="xs:integer"
                select="count(//*[util:contributesToRollup(.) and @ism:atomicEnergyMarkings and @ism:classification != 'U'])"/>
            <xsl:variable name="exceptions" as="xs:NMTOKEN*">
                <xsl:choose>
                    <xsl:when test="$countAEA != 0 and $countNATO != 0">
                        <xsl:copy-of select="xs:NMTOKEN('NATO-AEA')"/>
                    </xsl:when>
                    <xsl:when test="$countNATO != 0">
                        <xsl:copy-of select="xs:NMTOKEN('NATO')"/>
                    </xsl:when>
                    <xsl:when test="$countAEA != 0">
                        <xsl:copy-of select="xs:NMTOKEN('AEA')"/>
                    </xsl:when>
                </xsl:choose>
                <xsl:if test="count($distinctdeclassException) &gt; 0">
                    <xsl:copy-of select="$distinctdeclassException"/>
                </xsl:if>
            </xsl:variable>
            
            <xsl:if test="count(distinct-values($exceptions)) != 0 ">
                <xsl:attribute name="declassException" namespace="urn:us:gov:ic:ism">
                    <xsl:value-of select="ism-func:join(distinct-values($exceptions))"/>
             </xsl:attribute>
            </xsl:if>
            <xsl:if test="count($distinctdeclassEvent) != 0">
                <xsl:attribute name="declassEvent" namespace="urn:us:gov:ic:ism">
                    <xsl:value-of select="ism-func:join($distinctdeclassEvent)"/>
                </xsl:attribute>
            </xsl:if>
            <xsl:choose>
                <xsl:when test="count($distinctdeclassDate) != 0 and (count($exceptions) = 0 or not(normalize-space($exceptions)))">
                    <xsl:variable name="maxDate" select="max($distinctdeclassDate)"/>
                    <xsl:attribute name="declassDate" namespace="urn:us:gov:ic:ism">
                        <xsl:value-of select="string($maxDate)"/>
                    </xsl:attribute>
                </xsl:when>
                <xsl:when test="count($exceptions) = 0 ">
                    <xsl:attribute name="declassDate" namespace="urn:us:gov:ic:ism">
                        <xsl:value-of select="format-date(current-date() + xs:yearMonthDuration('P25Y'),'[Y0001]-[M01]-[D01]')"/>
                    </xsl:attribute>
                </xsl:when>
                <xsl:otherwise/>
            </xsl:choose>
        </xsl:if>
    </xsl:template>
    
    <xd:doc>
        <xd:desc>
            <xd:p>Create an ISM FGIsourceOpen, or FGIsourceProtected attribute based on values in the document. Assumes US document</xd:p>
            <xd:p>Handles the following  disseminationControls special cases</xd:p>
            <xd:ul>
                <xd:li>Any FGIsourceProtected supress all FGIsourceOpen</xd:li>
                <xd:li>Any @ism:ownerProducer not USA are added to FGIsourceOpen</xd:li>
            </xd:ul>
        </xd:desc>
    </xd:doc>
    <xsl:template name="AddISMFGI">
        <xsl:variable name="distinctFGIsourceOpen"
            select="distinct-values(//*[util:contributesToRollup(.) and @ism:FGIsourceOpen]/xs:NMTOKENS(@ism:FGIsourceOpen))"/>
        <xsl:variable name="distinctFGIsourceProtected"
            select="distinct-values(//*[util:contributesToRollup(.) and @ism:FGIsourceProtected]/xs:NMTOKENS(@ism:FGIsourceProtected))"/>
        <xsl:variable name="distinctownerProducer"
            select="distinct-values(//*[util:contributesToRollup(.) and @ism:ownerProducer != 'USA']/xs:NMTOKENS(@ism:ownerProducer))"/>
        
        <xsl:choose>
            <xsl:when test="(count($distinctFGIsourceProtected) != 0)">
                <xsl:attribute name="FGIsourceProtected" namespace="urn:us:gov:ic:ism">FGI</xsl:attribute>
            </xsl:when>
            <xsl:otherwise>
                <xsl:variable name="FGIUnion" as="xs:anyAtomicType*">
                    <xsl:sequence select="distinct-values(($distinctFGIsourceOpen, $distinctownerProducer[not(. = xs:NMTOKENS('USA'))]))"/>
                </xsl:variable>
                <xsl:if test="count($FGIUnion) != 0">
                    <xsl:attribute name="FGIsourceOpen" namespace="urn:us:gov:ic:ism">
                        <xsl:value-of select="ism-func:sortFGIOpen(ism-func:join($FGIUnion))"/>
                    </xsl:attribute>
                </xsl:if>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:template>
    
    <xd:doc>
        <xd:desc>
            <xd:p>Create an ISM nonUSControls attribute based on values in the document.</xd:p>
            <xd:p>There are no known special cases to handle at this time.</xd:p>
        </xd:desc>
    </xd:doc>
    <xsl:template name="AddISMnonUSControls">
        <xsl:variable name="distinctnonUSControls" select="distinct-values(//*[util:contributesToRollup(.) and @ism:nonUSControls]/xs:NMTOKENS(@ism:nonUSControls))"/>
        <xsl:if test="count($distinctnonUSControls) != 0">
            <xsl:attribute name="nonUSControls" namespace="urn:us:gov:ic:ism">
                <xsl:value-of select="ism-func:join($distinctnonUSControls)"/>
            </xsl:attribute>
        </xsl:if>
    </xsl:template>
    
    <xd:doc>
        <xd:desc>
            <xd:p>Create an ISM classification attribute based on values in the document.</xd:p>
            <xd:p>Handles the following special cases</xd:p>
            <xd:ul>
                <xd:li>R gets upgraded to C</xd:li>
                <xd:li>Lack of any valid token gets classification set to "InvalidClassificationInput" this is an intentional invalid marking to cause validation to fail on bad input. </xd:li>
            </xd:ul>
        </xd:desc>
    </xd:doc>
    <xsl:template name="addISMclassification">
        <xsl:attribute name="classification" namespace="urn:us:gov:ic:ism">
            <xsl:choose>
                <xsl:when test="key('classification', 'TS')">TS</xsl:when>
                <xsl:when test="key('classification', 'S')">S</xsl:when>
                <xsl:when test="key('classification', 'C')">C</xsl:when>
                <xsl:when test="key('classification', 'R')">C</xsl:when>
                <xsl:when test="key('classification', 'U')">U</xsl:when>
                <xsl:otherwise>InvalidClassificationInput</xsl:otherwise>
            </xsl:choose>
        </xsl:attribute>
    </xsl:template>

    <xd:doc>
        <xd:desc>
            <xd:p>Create an ISM SCIcontrols attribute based on values in the document. If there are no SCIcontrols don't create the attribute.</xd:p>
            <xd:p>There are no special cases for SCI just the distinct union of all sorted alphabetically</xd:p>
        </xd:desc>
    </xd:doc>
    <xsl:template name="AddISMSCIcontrols">
        <xsl:variable name="distinctSCI" select="distinct-values(//*[util:contributesToRollup(.) and @ism:SCIcontrols]/xs:NMTOKENS(@ism:SCIcontrols))" as="xs:NMTOKEN*"/>
        <xsl:if test="count($distinctSCI) !=0">
            <xsl:attribute name="SCIcontrols" namespace="urn:us:gov:ic:ism">
                <xsl:value-of select="ism-func:sortSciControls(ism-func:join($distinctSCI))"/>
            </xsl:attribute>
        </xsl:if>
    </xsl:template>

    <xd:doc>
        <xd:desc>
            <xd:p>Create an ISM atomicEnergyMarkings attribute based on values in the document. If there are no atomicEnergyMarkings don't create the
                attribute.</xd:p>
            <xd:p>Handles the following special cases</xd:p>
            <xd:ul>
                <xd:li>UCNI and DCNI are dropped from the banner of classified documents.</xd:li>
            </xd:ul>
        </xd:desc>
    </xd:doc>
    <xsl:template name="AddISMatomicEnergyMarkings">
        <xsl:variable name="distinctAtomic"
            select="distinct-values(//*[util:contributesToRollup(.) and @ism:atomicEnergyMarkings]/xs:NMTOKENS(@ism:atomicEnergyMarkings))"/>
        <xsl:variable name="unclassAtomic" select="xs:NMTOKENS('UCNI DCNI')"/>

        <xsl:variable name="atomicFiltered1">
            <xsl:choose>
                <xsl:when test="(key('classification', 'TS') | key('classification', 'S') | key('classification', 'C') | key('classification', 'R'))">
                    <xsl:copy-of select="$distinctAtomic[not(. = $unclassAtomic)]"/>
                </xsl:when>
                <xsl:otherwise>
                    <xsl:copy-of select="$distinctAtomic"/>
                </xsl:otherwise>
            </xsl:choose>
        </xsl:variable>

        <xsl:if test="normalize-space($atomicFiltered1)">
            <xsl:attribute name="atomicEnergyMarkings" namespace="urn:us:gov:ic:ism">
                <xsl:value-of select="ism-func:sortAtomicenergymarkings($atomicFiltered1)"/>
            </xsl:attribute>
        </xsl:if>
    </xsl:template>

    <xd:doc>
        <xd:desc>
            <xd:p>Create an ISM @ism:derivativelyClassifiedBy and @ism:derivedFrom attributes based partly on values in the document and partly on parameters
                passed in.</xd:p>
            <xd:p>Handles the following special cases</xd:p>
            <xd:ul>
                <xd:li>Unclassified documents don't get these attributes</xd:li>
                <xd:li>Classified documents get them set to the parameters passed.</xd:li>
            </xd:ul>
        </xd:desc>
        <xd:param name="derivedFrom">
            <xd:p>The source that should be cited in a derived from statement.</xd:p>
        </xd:param>
        <xd:param name="derivativelyClassifiedBy">
            <xd:p>The Person who is responsible for the decision. Since this is software it should be the accrediting human responsible for it being in
                production.</xd:p>
        </xd:param>
    </xd:doc>
    <xsl:template name="addISMderived">
        <xsl:param name="derivedFrom" required="yes"/>
        <xsl:param name="derivativelyClassifiedBy" required="yes"/>
        <xsl:if test="util:classifiedDoc(/)">
            <xsl:attribute name="derivativelyClassifiedBy" namespace="urn:us:gov:ic:ism" select="$derivativelyClassifiedBy"/>
            <xsl:attribute name="derivedFrom" namespace="urn:us:gov:ic:ism" select="$derivedFrom"/>
        </xsl:if>
    </xsl:template>

    <xd:doc>
        <xd:desc>
            <xd:p>Identity Transform uses mode="#current" to preserve whatever mode was used.</xd:p>
        </xd:desc>
    </xd:doc>
    <xsl:template match="@* | node()" mode="#default stripBlockAttributes identity">
        <xsl:copy>
            <xsl:apply-templates select="@* | node()" mode="#current"/>
        </xsl:copy>
    </xsl:template>

    <xd:doc>
        <xd:desc>
            <xd:p>Strip out block attributes for things like notice list that needed to have a rollup done but are not really resource nodes.</xd:p>
        </xd:desc>
    </xd:doc>
    <xsl:template match="@ism:resourceElement | @ism:derivedFrom | @ism:derivativelyClassifiedBy |@ism:declassDate" mode="stripBlockAttributes" priority="10"/>
    
    <xd:doc>
        <xd:desc>Normalize an ntk:AccessProfile by sorting all of the ntk:VocabularyType followed by all the ntk:AccessProfileValue</xd:desc>
    </xd:doc>
    <xsl:template match="ntk:AccessProfile" mode="normalizeNTKProfile">
        <xsl:copy>
            <xsl:apply-templates select="@*" mode="identity"/>
            <xsl:apply-templates select="ntk:AccessPolicy | ntk:ProfileDes" mode="identity"/>
            <xsl:apply-templates select="ntk:VocabularyType" mode="identity">
                <xsl:sort select="./@ntk:name" data-type="text" order="ascending" />
            </xsl:apply-templates>
            <xsl:apply-templates select="ntk:AccessProfileValue" mode="identity">
                <xsl:sort select="concat(./@ntk:qualifier,@ntk:vocabulary,string(.)) " data-type="text" order="ascending" />
            </xsl:apply-templates>
        </xsl:copy>
    </xsl:template>
    
    <xd:doc>
        <xd:desc>Normalize an ntk:AccessGroup by sorting all of the ntk:AccessGroupValue</xd:desc>
    </xd:doc>
    <xsl:template match="ntk:AccessGroup" mode="normalizeNTKProfile">
        <xsl:copy>
            <xsl:apply-templates select="@*" mode="identity"/>
            <xsl:apply-templates select="ntk:AccessPolicy | ntk:ProfileDes" mode="identity"/>
            <xsl:apply-templates select="ntk:AccessGroupValue" mode="identity">
                <xsl:sort select="." data-type="text" order="ascending" />
            </xsl:apply-templates>
        </xsl:copy>
    </xsl:template>
   
    <!--****************************-->
    <!-- (U) Custom XSLT functions   -->
    <!--****************************-->

    <xd:doc>
        <xd:desc>
            <xd:p>Returns true if the attribute @ism:excludeFromRollup is not present or does not evaluate to 'true'</xd:p>
        </xd:desc>
        <xd:param name="context"/>
    </xd:doc>
    <xsl:function name="util:contributesToRollup" as="xs:boolean">
        <xsl:param name="context"/>
        <xsl:sequence
            select="
            not($context/@ism:excludeFromRollup castable as xs:boolean and $context/@ism:excludeFromRollup = true()) and $context/@ism:ownerProducer
                and not(generate-id($context) =generate-id(util:resourceElement($context)) )"/>       
    </xsl:function>
    
    <xd:doc>
        <xd:desc>
            <xd:p>Returns true() if the document contains any classified portions that contribute to rollup.</xd:p>
        </xd:desc>
        <xd:param name="context">
            <xd:p>used by the Key function since this is in an xsl:function it's required. Just pass in "/"</xd:p>
        </xd:param>
    </xd:doc>
    <xsl:function name="util:classifiedDoc" as="xs:boolean">
        <xsl:param name="context"/>
        <xsl:choose>
            <xsl:when test="key('classification', 'TS', $context)">
                <xsl:value-of select="true()"/>
            </xsl:when>
            <xsl:when test="key('classification', 'S', $context)">
                <xsl:value-of select="true()"/>
            </xsl:when>
            <xsl:when test="key('classification', 'C', $context)">
                <xsl:value-of select="true()"/>
            </xsl:when>
            <xsl:when test="key('classification', 'R', $context)">
                <xsl:value-of select="true()"/>
            </xsl:when>
            <xsl:when test="key('classification', 'U', $context)">
                <xsl:value-of select="false()"/>
            </xsl:when>
            <xsl:otherwise>
                <xsl:message select="'Classification unknown value'"/>
                <xsl:value-of select="true()"/>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Checks that every portion has a particular Dissem Control.</xd:p>
        </xd:desc>
        <xd:param name="context"/>
        <xd:param name="distinctDisseminationControls"/>
        <xd:param name="Dissem"/>
    </xd:doc>
    <xsl:function name="util:allDissemPortionsHave" as="xs:boolean">
        <xsl:param name="context"/>
        <xsl:param name="distinctDisseminationControls"/>
        <xsl:param name="Dissem" as="xs:NMTOKEN*"/>
        <xsl:choose>
            <xsl:when test="$distinctDisseminationControls = $Dissem">
                <xsl:variable name="contributescount" select="count(root($context)//*[util:contributesToRollup(.) and util:PartakesInFDR(.)])"/>
                <xsl:variable name="contributesSpecifiedcount"
                    select="count(root($context)//*[util:contributesToRollup(.) and util:PartakesInFDR(.)][normalize-space(@ism:disseminationControls)][xs:NMTOKENS(@ism:disseminationControls) = $Dissem])"/>
                <xsl:choose>
                    <xsl:when test="$contributesSpecifiedcount = $contributescount">
                        <xsl:value-of select="true()"/>
                    </xsl:when>
                    <xsl:otherwise>
                        <xsl:value-of select="false()"/>
                    </xsl:otherwise>
                </xsl:choose>
            </xsl:when>
            <xsl:otherwise>
                <xsl:value-of select="false()"/>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:function>

    <xd:doc>
        <xd:desc> Given the a set of Rel strings return hash of those tokens count in the document  </xd:desc>
        <xd:param name="relToStrings"/>
    </xd:doc>
    <xsl:function name="util:createRelTokenHash" as="item()*">
        <xsl:param name="relToStrings" as="xs:string*"/>
        <xsl:for-each-group select="for $each in $relToStrings return distinct-values(ism-func:tokenize($each))" group-by=".">
            <xsl:sort select="current-grouping-key()"/>
            <token count='{count(current-group())}'><xsl:value-of select="current-grouping-key()"/></token>

        </xsl:for-each-group>
    </xsl:function>
    
    <xd:doc>
        <xd:desc>
            <xd:p>Cover function to allow calling without a count of portions.</xd:p>
        </xd:desc>
        <xd:param name="relToStrings"/>
    </xd:doc>
    <xsl:function name="util:commonToAllTokens">
        <xsl:param name="relToStrings" as="xs:string*"/>
        <xsl:sequence select="util:commonToAllTokens($relToStrings,$countFdrPortions)"></xsl:sequence>
    </xsl:function>
    
    <xd:doc>
        <xd:desc>
            <xd:p>Determine what values are commen to all portions for a REL to logic.</xd:p>
        </xd:desc>
        <xd:param name="relToStrings"/>
        <xd:param name="countFdrPortionsLocal"/>
    </xd:doc>
    <xsl:function name="util:commonToAllTokens">
        <xsl:param name="relToStrings" as="xs:string*"/>
        <xsl:param name="countFdrPortionsLocal" as="xs:integer"/>
        <xsl:variable name="RelTokenHash" select="util:createRelTokenHash($relToStrings)"/>
        <xsl:sequence select="$RelTokenHash[@count=$countFdrPortionsLocal]"/>
    </xsl:function>
    
    <xd:doc>
        <xd:desc> Remove the tokens that are common to all rel strings leaving us with the tokens that need expanding or are unmatched countries. </xd:desc>
        <xd:param name="relToStrings"/>
        <xd:param name="countFdrPortionsLocal"/>
    </xd:doc>
    <xsl:function name="util:RelStringsWithoutCommonTokens">
        <xsl:param name="relToStrings" as="xs:string*"/>
        <xsl:param name="countFdrPortionsLocal" as="xs:integer"/>
        <xsl:variable name="CommonTokens" select="util:commonToAllTokens($relToStrings,$countFdrPortionsLocal)"/>
        <xsl:for-each-group select="for $each in $relToStrings return ism-func:join(distinct-values(for $relToken in ism-func:tokenize($each) return (if (some $Token in $CommonTokens satisfies $Token=$relToken) then ' ' else ism-func:padValue($relToken) )))" group-by=".">
            <xsl:sequence  select="string(current-grouping-key())"/>
        </xsl:for-each-group>
    </xsl:function>

    <xd:doc>
        <xd:desc>
            <xd:p>Return the common tokens without any particular order.</xd:p>
        </xd:desc>
        <xd:param name="relToStrings"/>
        <xd:param name="countFdrPortionsLocal"/>
    </xd:doc>
    <xsl:function name="util:unsortedCommonRelTokens">
        <xsl:param name="relToStrings" as="xs:string*"/>
        <xsl:param name="countFdrPortionsLocal" as="xs:integer"/>
        <xsl:variable name="CommonTokens" select="util:commonToAllTokens($relToStrings,$countFdrPortionsLocal)"/>
        <xsl:variable name="relToMinusCommonTokens" select="util:RelStringsWithoutCommonTokens($relToStrings,$countFdrPortionsLocal)"/>
        <xsl:variable name="expandTetrasForRelTokensNotCommon" select="util:expandAllTetras($relToMinusCommonTokens)"/>
        <xsl:variable name="CommonTokensAfterExpansion" select="util:commonToAllTokens($expandTetrasForRelTokensNotCommon,$countFdrPortionsLocal)"/>
        <xsl:sequence select="$CommonTokens | $CommonTokensAfterExpansion"></xsl:sequence>
    </xsl:function>

    <!-- Maybe odd stuff lifted from ISM -->

    <xd:doc>
        <xd:desc> Given a sequence of $relToStrings (e.g. ('USA CAN GBR', 'USA AUS SPAA')), returns a set of tokens 
            that are each of these $relToStrings decomposed using ism-func:expandDecomposableTetras() </xd:desc>
        <xd:param name="relToStrings"/>
    </xd:doc>
    <xsl:function
        name="util:expandAllTetras"
        as="xs:string*">
        <xsl:param name="relToStrings" as="xs:string*"/>
        
        <xsl:variable name="allTokens" as="xs:string*">
            <xsl:for-each select="$relToStrings">
                <xsl:variable name="expandedCountryTokens" select="ism-func:expandDecomposableTetras(.)"/>
                <xsl:value-of select="ism-func:padValue(ism-func:join($expandedCountryTokens))"/>
            </xsl:for-each>
        </xsl:variable>
        
        <xsl:sequence select="$allTokens"/>
    </xsl:function>

    <xd:doc>
        <xd:desc> Recursively remove all decomposable tetragraphs in the given $relTo string 
            and replace them with their constituent countries. Note: Does not include USA </xd:desc>
        <xd:param name="relTo"/>
    </xd:doc>
    <!-- DY: refactored into IC-ISM-Functions.xsl -->
    <!--xsl:function
        name="util:expandDecomposableTetras"
        as="xs:string*">
        <xsl:param name="relTo" as="xs:string"/>
        
        <xsl:variable name="expandedTetras">
            <xsl:choose>
                <xsl:when test="util:containsDecomposableTetra($relTo)">
                    <xsl:variable name="currTetra"
                        select="util:tokenize($relTo)[. = $decomposableTetras][1]"/>
                    <xsl:variable name="currTetraCountries"
                        select="ism-func:join(util:getCountriesForTetra($currTetra))"/>
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
    </xsl:function-->

    <xd:doc>
        <xd:desc> Returns true if the given $relTo string (e.g. 'USA CAN GBR') contains any 
            tetragraphs that can be decomposed into its constituent countries  </xd:desc>
        <xd:param name="relTo"/>
    </xd:doc>
    <!-- DY: refactored into IC-ISM-Functions.xsl -->
    <!--xsl:function
        name="util:containsDecomposableTetra"
        as="xs:boolean">
        <xsl:param name="relTo" as="xs:string?"/>
        
        <xsl:sequence select="normalize-space($relTo) and util:containsAnyOfTheTokens($relTo, $decomposableTetras)"/>
    </xsl:function-->
    
    <xd:doc>
        <xd:desc>
            Returns true if any token in the attribute value matches at least one token in the provided list.
        </xd:desc>
        <xd:param name="attribute"/>
        <xd:param name="tokenList"/>
    </xd:doc>
    <!-- DY: refactored into IC-ISM-Functions.xsl -->
    <!--xsl:function
        name="util:containsAnyOfTheTokens"
        as="xs:boolean">
        <xsl:param name="attribute"/>
        <xsl:param name="tokenList" as="xs:string*"/>
        <xsl:sequence select="some $attrToken in tokenize(normalize-space(string($attribute)), ' ') satisfies $attrToken = $tokenList"/>
    </xsl:function-->
    
    <xd:doc>
        <xd:desc> Returns the sequence of country codes that correspond to the given $tetra </xd:desc>
        <xd:param name="tetra"/>
    </xd:doc>
    <!-- DY: refactored into IC-ISM-Functions.xsl -->
    <!--xsl:function
        name="util:getCountriesForTetra"
        as="xs:string*">
        <xsl:param name="tetra" as="xs:string"/>
        
        <xsl:sequence select="$decomposableTetraElems[catt:TetraToken/text() = $tetra]/catt:Membership/*/text()"/>
    </xsl:function-->
    
    <xd:doc>
        <xd:desc> Returns normalized $value with a preceding and subsequent space (' ') character </xd:desc>
        <xd:param name="value"/>
    </xd:doc>
    <!-- DY: refactored into IC-ISM-Functions.xsl -->
    <!--xsl:function
        name="util:padValue"
        as="xs:string">
        <xsl:param name="value" as="xs:string?"/>
        
        <xsl:value-of select="concat(' ', normalize-space($value), ' ')"/>
    </xsl:function-->
    
    <xd:doc>
        <xd:desc> Returns the given $value with its values broken into tokens using whitespace as delimiters </xd:desc>
        <xd:param name="value"/>
    </xd:doc>
    <!-- DY: refactored into IC-ISM-Functions.xsl -->
    <!--xsl:function
        name="util:tokenize"
        as="xs:string*">
        <xsl:param name="value" as="xs:string?"/>
        
        <xsl:sequence select="tokenize(normalize-space($value), ' ')"/>
    </xsl:function-->
    
    <xd:doc>
        <xd:desc>
            Accepts an element.
            Returns true if the element contains any Foreign Disclosure &amp; Release (FD&amp;R) markings; false otherwise.
        </xd:desc>
        <xd:param name="elementNode"/>
    </xd:doc>
    <xsl:function
        name="util:containsFDR"
        as="xs:boolean">
        <xsl:param name="elementNode" as="node()"/>
        <xsl:sequence select="$elementNode/@ism:releasableTo or $elementNode/@ism:displayOnlyTo or ism-func:containsAnyOfTheTokens($elementNode/@ism:disseminationControls, ('NF', 'RELIDO')) or ism-func:containsAnyOfTheTokens($elementNode/@ism:nonICmarkings, ('LES-NF', 'SBU-NF'))"/>
    </xsl:function>
    
    <xd:doc>
        <xd:desc>
            Returns true if the element would partake in an Foreign Disclosure &amp; Release (FD&amp;R)  rollup decision. 
        </xd:desc>
        <xd:param name="elementNode"/>
    </xd:doc>
    <xsl:function name="util:PartakesInFDR"  as="xs:boolean">
        <xsl:param name="elementNode" as="node()"/>
        <xsl:choose>
            <xsl:when test="$elementNode[@ism:classification = 'U']">
                <xsl:choose>
                    <!-- Uncaveated unclassified information does not impact FDR rollup. -->
                    <xsl:when test="$elementNode[not(@ism:disseminationControls) and not(@ism:nonICmarkings)]">
                        <xsl:value-of select="false()"/>
                    </xsl:when><xsl:otherwise>
                        <xsl:value-of select="true()"/>
                    </xsl:otherwise>   
                </xsl:choose>
                
            </xsl:when>
            <xsl:otherwise>
                <xsl:value-of select="true()"/>
            </xsl:otherwise>
            
        </xsl:choose>
    </xsl:function>
    
    <xd:doc>
        <xd:desc> Return all the tags that participate in rollup decisions.</xd:desc>
        <xd:param name="context"/>
    </xd:doc>
    <xsl:function name="util:partTags">
        <xsl:param name="context"/>
        <xsl:sequence select="$context/descendant-or-self::node()[@ism:classification and util:contributesToRollup(.)]"/>
    </xsl:function>
    
    <xd:doc>
        <xd:desc>
            Count how many of partTags participate in Foreign Disclosure &amp; Release (FD&amp;R) decisions. 
        </xd:desc>
        <xd:param name="context"/>
    </xd:doc>
    <xsl:function name="util:countFdrPortions">
        <xsl:param name="context"/>
        <xsl:value-of select="count(util:partTags($context)[util:PartakesInFDR(.)])"/>
    </xsl:function>
    
    <xd:doc>
        <xd:desc>
            <xd:p>The first element in document order that has resourceElement = true</xd:p>
        </xd:desc>
        <xd:param name="context"/>
    </xd:doc>
    <xsl:function name="util:resourceElement">
        <xsl:param name="context"/>
        <xsl:choose>
            <xsl:when test="exists($resourceElement)">
                <xsl:sequence select="$resourceElement"/>
            </xsl:when>
            <xsl:otherwise>
                <xsl:sequence select="util:resourceElementTraverse($context)"/>
            </xsl:otherwise>
        </xsl:choose>
    </xsl:function>
   
    <xsl:function name="util:resourceElementTraverse">
        <xsl:param name="context"/>
                <xsl:sequence select="(root($context)//*[@ism:resourceElement castable as xs:boolean and @ism:resourceElement = true() ] )[1]"/>
        
    </xsl:function>
</xsl:stylesheet>
