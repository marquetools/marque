<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:ism="urn:us:gov:ic:ism" xmlns:sch="http://purl.oclc.org/dsdl/schematron"
    id="ISM-ID-00451">
    <sch:p ism:classification="U" ism:ownerProducer="USA" class="ruleText"> 
        [ISM-ID-00451][Error] Every ARH attribute in the document must be specified with a non-whitespace value. 
        
        Human Readable: All attributes in the ARH namespace must specify a value. 
    </sch:p>
    <sch:p ism:classification="U" ism:ownerProducer="USA" class="codeDesc"> 
        For each element which specifies an attribute in the ARH namespace, ensure that all attributes in the ARH namespace
        contain a non-whitespace value. </sch:p>
    <sch:rule id="ISM-ID-00451-R1" context="*[@arh:*]">
        <sch:assert test="every $attribute in @arh:* satisfies string-length(normalize-space(string($attribute))) &gt; 0" flag="error" role="error"> 
            [ISM-ID-00451][Error] Every ARH attribute in the document must be specified with a non-whitespace value. 
            
            Human Readable: All attributes in the ARH namespace must specify a value. </sch:assert>
    </sch:rule>
</sch:pattern>
