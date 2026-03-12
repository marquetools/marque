<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00360">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00360][Error] An UNCLASSIFIED//FOUO tetragraph may not be used in a UNCLASSIFIED document that is not also FOUO.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For documents that use tetragraphs, this rule verifies that if a tetragraph is UNCLASSIFIED//FOUO, 
        and the document is UNCLASSIFIED, then the document must also be FOUO.
    </sch:p>
    <sch:rule id="ISM-ID-00360-R1" context="*[@ism:resourceElement=true()][1]">
        <sch:let name="documentClassification" value="@ism:classification"/>
        <sch:let name="documentIsFOUO" value="some $dissem in tokenize(@ism:disseminationControls, ' ') satisfies $dissem eq 'FOUO'"/>        
        <sch:let name="tetrasWithFOUO" value="distinct-values(for $value in $tetras return              if($catt//catt:Tetragraph[catt:TetraToken=$value]/@ism:ownerProducer and (some $dissem in tokenize($catt//catt:Tetragraph[catt:TetraToken=$value]/@ism:disseminationControls, ' ') satisfies $dissem eq 'FOUO'))              then $value             else null)"/>
        <sch:assert test="not($documentClassification = 'U' and not($documentIsFOUO) and not(empty($tetrasWithFOUO)))" flag="error" role="error">
            [ISM-ID-00360][Error] An UNCLASSIFIED document may not use FOUO tetragraphs unless the document is also FOUO.
            The following tetragraphs are UNCLASSIFIED//FOUO: 
            <sch:value-of select="string-join($tetrasWithFOUO,', ')"/>.
            Document classification:
            <sch:value-of select="$documentClassification"/>
            Document is FOUO:
            <sch:value-of select="$documentIsFOUO"/>
        </sch:assert>        
        <sch:assert test="exists($catt//catt:Tetragraphs)" flag="error" role="error">ISMCAT Taxonomy does not exist!</sch:assert>
    </sch:rule>
</sch:pattern>