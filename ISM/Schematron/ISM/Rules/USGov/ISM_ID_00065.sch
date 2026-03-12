<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00065">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00065][Error] If ISM_USGOV_RESOURCE and any element meeting ISM_CONTRIBUTES in the document 
        have the attribute @ism:FGIsourceProtected containing any value then the ISM_RESOURCE_ELEMENT 
        must have @ism:FGIsourceProtected with a value.
        
        Human Readable: USA documents having FGI Protected data must have FGI Protected at the resource level.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If IC Markings System Register and Manual rules do not apply to the document then the rule does not apply
        and this rule returns true. If any element has attribute @ism:FGIsourceProtected specified 
        with a non-empty value and does not have attribute @ism:excludeFromRollup set to true, 
        then this rule ensures that the banner has attribute @ism:FGIsourceProtected specified with 
        a non-empty value.
    </sch:p>  
    <sch:rule id="ISM-ID-00065-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(empty($partFGIsourceProtected))]">
        <sch:assert test="@ism:FGIsourceProtected" flag="error" role="error">
            [ISM-ID-00065][Error] If ISM_USGOV_RESOURCE and any element meeting ISM_CONTRIBUTES in the document 
            have the attribute @ism:FGIsourceProtected containing any value then the ISM_RESOURCE_ELEMENT 
            must have @ism:FGIsourceProtected with a value.
            
            Human Readable: USA documents having FGI Protected data must have FGI Protected at the resource level.
        </sch:assert>
    </sch:rule>
</sch:pattern>