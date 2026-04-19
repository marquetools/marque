<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00014">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00014][Error] If ISM_NSI_EO_APPLIES then one or more of the following 
        attributes: @ism:declassDate, @ism:declassEvent, or @ism:declassException must be specified on the ISM_RESOURCE_ELEMENT.
        Human Readable: Documents under E.O. 13526 must have declassification instructions included in the 
        classification authority block information.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If ISM_NSI_EO_APPLIES, this rule ensures that the ISM_RESOURCE_ELEMENT specifies
        one of the following attributes: @ism:declassDate, @ism:declassEvent, @ism:declassException.
    </sch:p>
    <sch:rule id="ISM-ID-00014-R1" context="*[$ISM_NSI_EO_APPLIES and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]">
        <sch:assert test="@ism:declassDate or @ism:declassEvent or @ism:declassException" flag="error" role="error">
            [ISM-ID-00014][Error] If ISM_NSI_EO_APPLIES then one or more of the following 
            attributes: @ism:declassDate, @ism:declassEvent, or @ism:declassException must be specified on the ISM_RESOURCE_ELEMENT.
            Human Readable: Documents under E.O. 13526 must have declassification instructions included in the 
            classification authority block information.
        </sch:assert>
    </sch:rule>
</sch:pattern>