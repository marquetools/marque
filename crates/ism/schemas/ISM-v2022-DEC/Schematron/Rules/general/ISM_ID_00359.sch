<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00359">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00359][Error] The classification of a tetragraph may not be greater than the classification of the document.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For documents that use tetragraphs, this rule verifies that the classification of the tetragraph isn't greater
        than the classification of the document.
    </sch:p>
    <sch:rule id="ISM-ID-00359-R1" context="*[@ism:resourceElement=true()][1]">
        <sch:let name="documentClassification" value="@ism:classification"/>
        <sch:let name="moreRestrictiveTetras" value="for $tetra in $tetras return              if ($catt//catt:Tetragraph[catt:TetraToken=$tetra]/@ism:classification != $documentClassification)              then             (if ($documentClassification = 'TS')             then null             else if ($catt//catt:Tetragraph[catt:TetraToken=$tetra]/@ism:classification = 'TS')             then $tetra             else if ($documentClassification = 'U')             then $tetra             else if ($documentClassification = 'C' and $catt//catt:Tetragraph[catt:TetraToken=$tetra]/@ism:classification = 'S')             then $tetra             else if ($documentClassification = 'R' and ($catt//catt:Tetragraph[catt:TetraToken=$tetra]/@ism:classification = 'C' or $catt//catt:Tetragraph[catt:TetraToken=$tetra]/@ism:classification = 'S'))             then $tetra             else             null             )             else null"/>  
        <sch:assert test="empty($moreRestrictiveTetras)" flag="error" role="error">
            [ISM-ID-00359][Error] A document using tetragraphs may not have a classification that is greater
            than the classification of the document. The following tetragraphs have a more restrictive classification
            than the document: <sch:value-of select="string-join($moreRestrictiveTetras,', ')"/>.
        </sch:assert>
        <sch:assert test="exists($catt//catt:Tetragraphs)" flag="error" role="error">ISMCAT Taxonomy does not exist!</sch:assert>
    </sch:rule>
</sch:pattern>